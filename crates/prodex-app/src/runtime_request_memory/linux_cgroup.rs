use std::io;
use std::path::{Component, Path, PathBuf};

// This runs in the launch/sampling worker, never on a request path.
pub(super) fn available(
    groups: &str,
    mounts: &str,
    mut read: impl FnMut(&Path) -> io::Result<String>,
) -> Result<Option<u64>, ()> {
    let mut unified = None;
    let mut legacy = None;
    for line in groups.lines() {
        let mut fields = line.splitn(3, ':');
        let _hierarchy = fields.next();
        let controllers = fields.next().ok_or(())?;
        let path = fields.next().ok_or(())?;
        if controllers.is_empty() {
            unified = Some(path);
        } else if controllers
            .split(',')
            .any(|controller| controller == "memory")
        {
            legacy = Some(path);
        }
    }
    let (path, filesystem, limit_name, current_name) = match (legacy, unified) {
        (Some(path), _) => (
            path,
            "cgroup",
            "memory.limit_in_bytes",
            "memory.usage_in_bytes",
        ),
        (None, Some(path)) => (path, "cgroup2", "memory.max", "memory.current"),
        (None, None) => return Ok(None),
    };
    let path = Path::new(path);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(());
    }
    let (mount_root, mountpoint) = mounts
        .lines()
        .find_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let mut post = after.split_whitespace();
            if post.next()? != filesystem {
                return None;
            }
            let _source = post.next()?;
            let options = post.next()?;
            if filesystem == "cgroup" && !options.split(',').any(|option| option == "memory") {
                return None;
            }
            let mut fields = before.split_whitespace();
            let root = unescape(fields.nth(3)?)?;
            let mountpoint = unescape(fields.next()?)?;
            Some((PathBuf::from(root), PathBuf::from(mountpoint)))
        })
        .ok_or(())?;
    if !mount_root.is_absolute()
        || !mountpoint.is_absolute()
        || mount_root
            .components()
            .chain(mountpoint.components())
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(());
    }
    let relative = path
        .strip_prefix(&mount_root)
        .or_else(|_| path.strip_prefix("/"))
        .map_err(|_| ())?;
    let mut directory = mountpoint.join(relative);
    let mut remaining: Option<u64> = None;
    let mut readable = false;
    loop {
        match read(&directory.join(limit_name)) {
            Ok(value) => {
                readable = true;
                if value.trim() != "max" {
                    let limit = value.trim().parse::<u64>().map_err(|_| ())?;
                    let current = read(&directory.join(current_name))
                        .map_err(|_| ())?
                        .trim()
                        .parse::<u64>()
                        .map_err(|_| ())?;
                    let capacity = limit.saturating_sub(current);
                    remaining = Some(remaining.map_or(capacity, |previous| previous.min(capacity)));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(()),
        }
        if directory == mountpoint || !directory.pop() {
            break;
        }
    }
    if readable { Ok(remaining) } else { Err(()) }
}

fn unescape(value: &str) -> Option<String> {
    let mut output = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            let a = bytes.next()?.checked_sub(b'0')?;
            let b = bytes.next()?.checked_sub(b'0')?;
            let c = bytes.next()?.checked_sub(b'0')?;
            if a > 7 || b > 7 || c > 7 {
                return None;
            }
            output.push(a.checked_mul(64)?.checked_add(b * 8)?.checked_add(c)?);
        } else {
            output.push(byte);
        }
    }
    String::from_utf8(output).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn container_budget_honors_tightest_parent_and_mount_namespace() {
        let files = BTreeMap::from([
            ("/test/cgroup/child/memory.max", "1000"),
            ("/test/cgroup/child/memory.current", "200"),
            ("/test/cgroup/memory.max", "900"),
            ("/test/cgroup/memory.current", "300"),
        ]);
        let read = |path: &Path| {
            files
                .get(path.to_str().unwrap())
                .map(|value| value.to_string())
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        };
        let mounts = "1 0 0:1 /parent /test/cgroup rw - cgroup2 cgroup rw";
        assert_eq!(available("0::/parent/child", mounts, read), Ok(Some(600)));
        assert_eq!(available("0::/child", mounts, read), Ok(Some(600)));
        assert_eq!(available("0::/../escape", mounts, read), Err(()));
        assert_eq!(available("0::/child", "", read), Err(()));
    }

    #[test]
    fn legacy_memory_controller_wins_in_hybrid_hierarchy() {
        let groups = "0::/unified\n3:cpu,memory:/worker";
        let mounts = "1 0 0:1 / /test/v2 rw - cgroup2 cgroup rw\n2 0 0:2 / /test/v1 rw - cgroup cgroup rw,memory";
        let files = BTreeMap::from([
            ("/test/v1/worker/memory.limit_in_bytes", "100"),
            ("/test/v1/worker/memory.usage_in_bytes", "40"),
            ("/test/v1/memory.limit_in_bytes", "max"),
        ]);
        let read = |path: &Path| {
            files
                .get(path.to_str().unwrap())
                .map(|value| value.to_string())
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        };
        assert_eq!(available(groups, mounts, read), Ok(Some(60)));
    }
}
