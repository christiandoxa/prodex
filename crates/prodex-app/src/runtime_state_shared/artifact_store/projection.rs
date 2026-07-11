use super::super::{
    RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
    RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION, RuntimeSmartContextArtifactRepoMapKey,
    runtime_smart_context_insert_repo_map_entry, runtime_smart_context_limited_repo_map,
    runtime_smart_context_push_projection_source_ranges,
    runtime_smart_context_repo_map_first_path_range,
    runtime_smart_context_repo_map_module_from_path, runtime_smart_context_repo_map_nearest_path,
    runtime_smart_context_repo_map_paths, runtime_smart_context_repo_map_symbol_kind,
    runtime_smart_context_repo_map_symbol_module,
};
use super::{
    RuntimeSmartContextArtifactProjectionPrewarm, RuntimeSmartContextArtifactRepoMap,
    RuntimeSmartContextArtifactRepoMapEntry, RuntimeSmartContextArtifactRepoMapEntryKind,
    RuntimeSmartContextArtifactStore,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeSmartContextArtifactProjectionKind {
    Repo,
    Symbol,
}

impl RuntimeSmartContextArtifactProjectionKind {
    fn includes_paths(self) -> bool {
        self == Self::Repo
    }
}

impl RuntimeSmartContextArtifactStore {
    pub(crate) fn repo_map_projection(&self, limit: usize) -> RuntimeSmartContextArtifactRepoMap {
        self.map_projection_from_prewarm_or_build(
            limit,
            self.repo_map_prewarm.as_ref(),
            RuntimeSmartContextArtifactProjectionKind::Repo,
        )
    }

    pub(crate) fn symbol_map_projection(&self, limit: usize) -> RuntimeSmartContextArtifactRepoMap {
        self.map_projection_from_prewarm_or_build(
            limit,
            self.symbol_map_prewarm.as_ref(),
            RuntimeSmartContextArtifactProjectionKind::Symbol,
        )
    }

    fn map_projection_from_prewarm_or_build(
        &self,
        limit: usize,
        prewarm: Option<&RuntimeSmartContextArtifactProjectionPrewarm>,
        projection_kind: RuntimeSmartContextArtifactProjectionKind,
    ) -> RuntimeSmartContextArtifactRepoMap {
        let limit = limit.min(RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES);
        if limit == 0 {
            return RuntimeSmartContextArtifactRepoMap {
                complete: self.artifacts.is_empty(),
                entries: Vec::new(),
            };
        }

        if let Some(prewarm) = prewarm
            && self.projection_prewarm_valid(prewarm)
        {
            return runtime_smart_context_limited_repo_map(prewarm.projection.clone(), limit);
        }

        self.build_map_projection(limit, projection_kind)
    }

    fn build_map_projection(
        &self,
        limit: usize,
        projection_kind: RuntimeSmartContextArtifactProjectionKind,
    ) -> RuntimeSmartContextArtifactRepoMap {
        let mut entries = BTreeMap::<
            RuntimeSmartContextArtifactRepoMapKey,
            RuntimeSmartContextArtifactRepoMapEntry,
        >::new();
        let mut complete = true;
        let mut artifacts = self.artifacts.values().collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            right
                .sequence
                .cmp(&left.sequence)
                .then_with(|| left.id.cmp(&right.id))
        });

        for artifact in artifacts {
            let Some(line_index) = artifact.line_index.as_ref() else {
                complete = false;
                continue;
            };
            complete &= line_index.semantic_complete && line_index.symbol_complete;

            let paths = runtime_smart_context_repo_map_paths(line_index);
            let primary_path = (paths.len() == 1).then(|| paths[0].clone());
            if projection_kind.includes_paths() {
                for path in paths {
                    let first_path_range =
                        runtime_smart_context_repo_map_first_path_range(line_index, &path);
                    runtime_smart_context_insert_repo_map_entry(
                        &mut entries,
                        RuntimeSmartContextArtifactRepoMapEntry {
                            kind: RuntimeSmartContextArtifactRepoMapEntryKind::Path,
                            module: runtime_smart_context_repo_map_module_from_path(&path),
                            symbol: None,
                            code: None,
                            line: first_path_range.and_then(|range| {
                                range.line.or(range.new_start).or(range.old_start)
                            }),
                            path: Some(path),
                            artifact_id: artifact.id.clone(),
                            sequence: artifact.sequence,
                            range_start: first_path_range.map_or(0, |range| range.start),
                            range_end: first_path_range.map_or(0, |range| range.end),
                        },
                    );
                }
            }

            for range in &line_index.symbol_ranges {
                let Some(symbol) = range.symbol.clone() else {
                    continue;
                };
                let kind = runtime_smart_context_repo_map_symbol_kind(range);
                let path = runtime_smart_context_repo_map_nearest_path(line_index, range.start)
                    .or_else(|| primary_path.clone());
                runtime_smart_context_insert_repo_map_entry(
                    &mut entries,
                    RuntimeSmartContextArtifactRepoMapEntry {
                        kind,
                        module: runtime_smart_context_repo_map_symbol_module(
                            kind,
                            path.as_deref(),
                            &symbol,
                        ),
                        symbol: Some(symbol),
                        code: None,
                        line: range.line,
                        path,
                        artifact_id: artifact.id.clone(),
                        sequence: artifact.sequence,
                        range_start: range.start,
                        range_end: range.end,
                    },
                );
            }

            for range in &line_index.error_ranges {
                let Some(code) = range.code.clone() else {
                    continue;
                };
                let path = runtime_smart_context_repo_map_nearest_path(line_index, range.start)
                    .or_else(|| primary_path.clone());
                runtime_smart_context_insert_repo_map_entry(
                    &mut entries,
                    RuntimeSmartContextArtifactRepoMapEntry {
                        kind: RuntimeSmartContextArtifactRepoMapEntryKind::Error,
                        module: path
                            .as_deref()
                            .and_then(runtime_smart_context_repo_map_module_from_path),
                        symbol: None,
                        code: Some(code),
                        line: Some(range.start),
                        path,
                        artifact_id: artifact.id.clone(),
                        sequence: artifact.sequence,
                        range_start: range.start,
                        range_end: range.end,
                    },
                );
            }
        }

        let mut entries = entries.into_values().collect::<Vec<_>>();
        if entries.len() > limit {
            entries.truncate(limit);
            complete = false;
        }

        RuntimeSmartContextArtifactRepoMap { complete, entries }
    }

    pub(in crate::runtime_state_shared::artifact_store) fn invalidate_prewarmed_projections(
        &mut self,
    ) {
        self.repo_map_prewarm = None;
        self.symbol_map_prewarm = None;
    }

    pub(in crate::runtime_state_shared::artifact_store) fn refresh_prewarmed_projections(
        &mut self,
    ) {
        let source_hash = self.projection_source_hash();
        let repo_valid = self
            .repo_map_prewarm
            .as_ref()
            .is_some_and(|prewarm| self.projection_prewarm_valid_for_source(prewarm, &source_hash));
        if !repo_valid {
            self.repo_map_prewarm = Some(RuntimeSmartContextArtifactProjectionPrewarm {
                schema_version: RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION,
                source_hash: source_hash.clone(),
                limit: RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                projection: self.repo_map_projection(RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES),
            });
        }

        let symbol_valid = self
            .symbol_map_prewarm
            .as_ref()
            .is_some_and(|prewarm| self.projection_prewarm_valid_for_source(prewarm, &source_hash));
        if !symbol_valid {
            self.symbol_map_prewarm = Some(RuntimeSmartContextArtifactProjectionPrewarm {
                schema_version: RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION,
                source_hash,
                limit: RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                projection: self.symbol_map_projection(RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES),
            });
        }
    }

    fn projection_prewarm_valid(
        &self,
        prewarm: &RuntimeSmartContextArtifactProjectionPrewarm,
    ) -> bool {
        let source_hash = self.projection_source_hash();
        self.projection_prewarm_valid_for_source(prewarm, &source_hash)
    }

    fn projection_prewarm_valid_for_source(
        &self,
        prewarm: &RuntimeSmartContextArtifactProjectionPrewarm,
        source_hash: &str,
    ) -> bool {
        prewarm.schema_version == RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION
            && prewarm.limit >= RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES
            && prewarm.source_hash == source_hash
    }

    fn projection_source_hash(&self) -> String {
        let mut source = String::new();
        let _ = writeln!(
            source,
            "schema={}",
            RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION
        );
        for artifact in self.artifacts.values() {
            let _ = writeln!(
                source,
                "artifact\t{}\t{}\t{}\t{}",
                artifact.id, artifact.content_hash, artifact.byte_len, artifact.sequence
            );
            let Some(line_index) = artifact.line_index.as_ref() else {
                source.push_str("line_index\tmissing\n");
                continue;
            };
            let _ = writeln!(
                source,
                "line_index\t{}\t{}\t{}\t{}\t{}",
                line_index.semantic_schema_version,
                line_index.complete,
                line_index.semantic_complete,
                line_index.symbol_complete,
                line_index.command_kind.as_deref().unwrap_or_default()
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "file",
                &line_index.file_location_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "diff",
                &line_index.diff_hunk_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "test",
                &line_index.test_failure_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "error",
                &line_index.error_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "symbol",
                &line_index.symbol_ranges,
            );
        }
        runtime_proxy_crate::smart_context_hash_text(&source)
    }
}
