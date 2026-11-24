# Requirement register: dwg-mcp

Every test carries the id of the requirement it checks, as a
`/* REQ-MCP-<id>: <description> */` comment. This file is generated from those
comments by `scripts/gen_requirements.py`. Allocate the next free number in the
block of the tool the requirement belongs to.

| Block | Ids |
|-------|-----|
| Server and tool definitions | REQ-MCP-001 to 029 |
| parse_file | REQ-MCP-030 to 059 |
| Result cache and reset_file_cache | REQ-MCP-060 to 089 |
| get_result | REQ-MCP-090 to 129 |
| get_job, list_jobs, delete_job | REQ-MCP-130 to 149 |
| parse_url and reset_url_cache | REQ-MCP-150 to 199 |
| DXF input | REQ-MCP-200 to 219 |
| Pure rules (unit tests) | REQ-MCP-U01 onwards |

## Server and tool definitions

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-001 | the server identifies itself by name, title and version | `tests/mcp_definitions.rs::server_reports_name_title_and_version` |
| REQ-MCP-002 | the server instructions name the entry tools and the cache rule | `tests/mcp_definitions.rs::server_instructions_name_the_entry_tools` |
| REQ-MCP-003 | the server advertises the tools capability | `tests/mcp_definitions.rs::server_advertises_tools_capability` |
| REQ-MCP-004 | tools/list returns exactly the eight agreed tools | `tests/mcp_definitions.rs::lists_exactly_the_eight_tools` |
| REQ-MCP-005 | every tool name is verb_noun in snake_case with a known verb | `tests/mcp_definitions.rs::tool_names_are_verb_noun_snake_case` |
| REQ-MCP-006 | every tool has a display title that says more than its name | `tests/mcp_definitions.rs::every_tool_has_a_meaningful_title` |
| REQ-MCP-007 | every description names a sibling tool to use instead | `tests/mcp_definitions.rs::every_description_names_an_alternative_tool` |
| REQ-MCP-008 | descriptions start with what the tool does and stay bounded | `tests/mcp_definitions.rs::descriptions_are_front_loaded_and_bounded` |
| REQ-MCP-009 | every input parameter carries a description | `tests/mcp_definitions.rs::every_input_parameter_is_described` |
| REQ-MCP-010 | input schemas are closed objects | `tests/mcp_definitions.rs::input_schemas_reject_unknown_properties` |
| REQ-MCP-011 | every tool declares an output schema | `tests/mcp_definitions.rs::every_tool_declares_an_output_schema` |
| REQ-MCP-012 | every tool declares all four behaviour hints | `tests/mcp_definitions.rs::every_tool_declares_all_four_hints` |
| REQ-MCP-013 | only the three reading tools are read-only | `tests/mcp_definitions.rs::reading_tools_are_read_only` |
| REQ-MCP-014 | delete and reset tools are destructive, parse tools are not | `tests/mcp_definitions.rs::deleting_tools_are_marked_destructive` |
| REQ-MCP-015 | only parse_url reaches outside the machine | `tests/mcp_definitions.rs::only_parse_url_is_open_world` |
| REQ-MCP-016 | parse tools and delete_job are not idempotent; reset tools are | `tests/mcp_definitions.rs::idempotency_hints_match_behaviour` |
| REQ-MCP-017 | each tool requires only its identifying argument | `tests/mcp_definitions.rs::required_arguments_are_minimal` |
| REQ-MCP-018 | get_result publishes the supported result modes in the schema | `tests/mcp_definitions.rs::get_result_schema_lists_the_result_modes` |
| REQ-MCP-019 | the parse tools share one output schema | `tests/mcp_definitions.rs::parse_tools_share_one_output_shape` |
| REQ-MCP-020 | calling a tool that does not exist is a protocol error | `tests/mcp_definitions.rs::unknown_tool_is_a_protocol_error` |
| REQ-MCP-021 | an argument that is not in the schema is rejected | `tests/mcp_definitions.rs::unknown_argument_is_rejected` |
| REQ-MCP-022 | a missing required argument is rejected and named | `tests/mcp_definitions.rs::missing_required_argument_is_rejected` |
| REQ-MCP-023 | an argument of the wrong type is rejected | `tests/mcp_definitions.rs::wrong_argument_type_is_rejected` |
| REQ-MCP-024 | the server survives a failed call and answers the next one | `tests/mcp_definitions.rs::server_keeps_serving_after_errors` |

## parse_file

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-030 | parse_file returns a completed job with a job_id | `tests/mcp_parse_file.rs::returns_a_completed_job` |
| REQ-MCP-031 | a first parse is not cached and costs one unit | `tests/mcp_parse_file.rs::first_parse_costs_one_unit` |
| REQ-MCP-032 | the summary lists Model space first, then the paper sheets | `tests/mcp_parse_file.rs::summary_lists_model_space_and_sheets` |
| REQ-MCP-033 | the summary counts labels per page and in total | `tests/mcp_parse_file.rs::summary_counts_labels` |
| REQ-MCP-034 | the summary names the layers and the DWG version | `tests/mcp_parse_file.rs::summary_names_layers_and_version` |
| REQ-MCP-035 | by default pages are 3000 px on the long edge, with tiles | `tests/mcp_parse_file.rs::defaults_are_3000_px_with_tiles` |
| REQ-MCP-036 | size sets the long edge of the page images | `tests/mcp_parse_file.rs::size_sets_the_long_edge` |
| REQ-MCP-037 | a size outside 512..12000 is refused before any parsing | `tests/mcp_parse_file.rs::size_out_of_range_is_refused` |
| REQ-MCP-038 | the size limits themselves are accepted | `tests/mcp_parse_file.rs::size_limits_are_inclusive` |
| REQ-MCP-039 | tiles=false produces no tile grid | `tests/mcp_parse_file.rs::tiles_false_produces_no_tiles` |
| REQ-MCP-040 | a relative path is refused | `tests/mcp_parse_file.rs::relative_path_is_refused` |
| REQ-MCP-041 | a path with no file is not_found | `tests/mcp_parse_file.rs::missing_file_is_not_found` |
| REQ-MCP-042 | a directory is refused | `tests/mcp_parse_file.rs::directory_is_refused` |
| REQ-MCP-043 | a file that is not named .dwg or .dxf is unsupported | `tests/mcp_parse_file.rs::other_extension_is_unsupported` |
| REQ-MCP-044 | a .dwg or .dxf file whose content is not a drawing is unsupported | `tests/mcp_parse_file.rs::non_dwg_content_is_unsupported` |
| REQ-MCP-045 | the .dwg extension is matched without regard to case | `tests/mcp_parse_file.rs::uppercase_extension_is_accepted` |
| REQ-MCP-046 | the job records its source path and the SHA-256 of the file | `tests/mcp_parse_file.rs::job_records_source_and_content_hash` |
| REQ-MCP-047 | a result expires exactly seven days after it was created | `tests/mcp_parse_file.rs::result_expires_seven_days_after_creation` |
| REQ-MCP-048 | a drawing without text parses with zero labels | `tests/mcp_parse_file.rs::drawing_without_text_has_no_labels` |
| REQ-MCP-049 | a damaged drawing fails with parse_failed instead of a placeholder | `tests/mcp_parse_file.rs::damaged_drawing_fails_to_parse` |
| REQ-MCP-050 | a failed parse is recorded as a failed job that costs nothing | `tests/mcp_parse_file.rs::failed_parse_is_recorded_and_free` |
| REQ-MCP-051 | a failed parse is not cached: the next attempt parses again | `tests/mcp_parse_file.rs::failed_parse_is_not_cached` |
| REQ-MCP-052 | the text content of a result is the same JSON as the structured content | `tests/mcp_parse_file.rs::text_content_mirrors_structured_content` |

## Result cache and reset_file_cache

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-060 | parsing the same file again is answered from the cache | `tests/mcp_cache.rs::second_parse_is_cached` |
| REQ-MCP-061 | a cache hit costs one tenth of a fresh parse | `tests/mcp_cache.rs::cache_hit_is_ten_times_cheaper` |
| REQ-MCP-062 | a cache hit is a new job with the same summary | `tests/mcp_cache.rs::cache_hit_is_a_new_job_with_the_same_summary` |
| REQ-MCP-063 | the cache is keyed by content, not by path or file name | `tests/mcp_cache.rs::cache_is_keyed_by_content` |
| REQ-MCP-064 | a different size is a different cache entry | `tests/mcp_cache.rs::different_size_is_a_fresh_parse` |
| REQ-MCP-065 | a different tiles setting is a different cache entry | `tests/mcp_cache.rs::different_tiles_setting_is_a_fresh_parse` |
| REQ-MCP-066 | omitted options and their explicit defaults are the same entry | `tests/mcp_cache.rs::explicit_defaults_hit_the_default_entry` |
| REQ-MCP-067 | an edited file at the same path parses fresh | `tests/mcp_cache.rs::edited_file_parses_fresh` |
| REQ-MCP-068 | the cache and its jobs survive a server restart | `tests/mcp_cache.rs::cache_survives_a_restart` |
| REQ-MCP-069 | a cache hit does not extend the life of the result | `tests/mcp_cache.rs::cache_hit_keeps_the_original_expiry` |
| REQ-MCP-070 | a result is still served on its sixth day | `tests/mcp_cache.rs::result_is_cached_within_seven_days` |
| REQ-MCP-071 | after seven days the result is gone and the parse is fresh | `tests/mcp_cache.rs::result_expires_after_seven_days` |
| REQ-MCP-072 | an expired job is not listed and cannot be read | `tests/mcp_cache.rs::expired_job_is_gone` |
| REQ-MCP-073 | expiry removes the cached files from disk | `tests/mcp_cache.rs::expiry_removes_files_from_disk` |
| REQ-MCP-074 | a cache hit does not render again | `tests/mcp_cache.rs::cache_hit_does_not_render_again` |
| REQ-MCP-075 | after reset_file_cache the next parse is a full parse | `tests/mcp_cache.rs::reset_file_cache_forces_a_fresh_parse` |
| REQ-MCP-076 | reset_file_cache drops every size and tile variant and their jobs | `tests/mcp_cache.rs::reset_file_cache_counts_what_it_removed` |
| REQ-MCP-077 | a job removed by reset_file_cache can no longer be read | `tests/mcp_cache.rs::reset_file_cache_invalidates_job_ids` |
| REQ-MCP-078 | reset_file_cache is idempotent and harmless for an unparsed file | `tests/mcp_cache.rs::reset_file_cache_twice_removes_nothing` |
| REQ-MCP-079 | reset_file_cache leaves other drawings cached | `tests/mcp_cache.rs::reset_file_cache_leaves_other_drawings` |
| REQ-MCP-080 | reset_file_cache refuses a relative path and a missing file | `tests/mcp_cache.rs::reset_file_cache_checks_the_path` |

## get_result

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-090 | without a mode, get_result returns the summary of the parse | `tests/mcp_get_result.rs::default_mode_is_the_summary` |
| REQ-MCP-091 | an unknown job_id is not_found and points to list_jobs | `tests/mcp_get_result.rs::unknown_job_is_not_found` |
| REQ-MCP-092 | mode index returns layers, layouts and the entity histogram | `tests/mcp_get_result.rs::index_lists_layers_layouts_and_entity_counts` |
| REQ-MCP-093 | the index carries a sample of the drawing's text | `tests/mcp_get_result.rs::index_samples_text` |
| REQ-MCP-094 | mode labels returns every label of the drawing with a total | `tests/mcp_get_result.rs::labels_returns_all_labels` |
| REQ-MCP-095 | labels carry the exact text written in the drawing | `tests/mcp_get_result.rs::labels_are_the_exact_text_of_the_drawing` |
| REQ-MCP-096 | the page filter keeps the labels of one page | `tests/mcp_get_result.rs::labels_filter_by_page` |
| REQ-MCP-097 | the layer filter matches the layer name without regard to case | `tests/mcp_get_result.rs::labels_filter_by_layer` |
| REQ-MCP-098 | the query filter is a case-insensitive substring match | `tests/mcp_get_result.rs::labels_filter_by_query` |
| REQ-MCP-099 | page, layer and query filters combine | `tests/mcp_get_result.rs::label_filters_combine` |
| REQ-MCP-100 | a filter that matches nothing returns an empty list, not an error | `tests/mcp_get_result.rs::labels_with_no_match_are_empty` |
| REQ-MCP-101 | label boxes lie inside their page image and have a size | `tests/mcp_get_result.rs::label_boxes_lie_inside_the_page` |
| REQ-MCP-102 | a label carries its layer and the layer's color | `tests/mcp_get_result.rs::labels_carry_layer_and_color` |
| REQ-MCP-103 | labels are paged, 100 per page by default | `tests/mcp_get_result.rs::labels_page_at_100_by_default` |
| REQ-MCP-104 | following next_cursor visits every label exactly once | `tests/mcp_get_result.rs::cursor_walks_every_label_once` |
| REQ-MCP-105 | limit accepts 1 to 500 and nothing else | `tests/mcp_get_result.rs::label_limit_is_bounded` |
| REQ-MCP-106 | a cursor that this tool did not issue is refused; one past the end is empty | `tests/mcp_get_result.rs::label_cursor_is_checked` |
| REQ-MCP-107 | a page that does not exist is refused with the valid range | `tests/mcp_get_result.rs::page_out_of_range_is_refused` |
| REQ-MCP-108 | page_image returns the page as PNG image content of the reported size | `tests/mcp_get_result.rs::page_image_returns_a_png` |
| REQ-MCP-109 | the image path points at the same PNG that was returned inline | `tests/mcp_get_result.rs::image_path_holds_the_same_png` |
| REQ-MCP-110 | page_image and tile_image require a page | `tests/mcp_get_result.rs::image_modes_require_a_page` |
| REQ-MCP-111 | tile_image returns one tile as a PNG with the labels inside it | `tests/mcp_get_result.rs::tile_image_returns_a_tile_and_its_labels` |
| REQ-MCP-112 | tiles are offset in the page by their column and row | `tests/mcp_get_result.rs::tiles_are_offset_within_the_page` |
| REQ-MCP-113 | every label of a page is inside at least one of its tiles | `tests/mcp_get_result.rs::tiles_together_cover_every_label` |
| REQ-MCP-114 | tile_image requires row and col inside the tile grid | `tests/mcp_get_result.rs::tile_coordinates_are_checked` |
| REQ-MCP-115 | tile_image on a drawing parsed without tiles says how to get tiles | `tests/mcp_get_result.rs::tile_image_needs_a_parse_with_tiles` |
| REQ-MCP-116 | reading a result creates no job and costs nothing | `tests/mcp_get_result.rs::reading_costs_nothing` |
| REQ-MCP-117 | a cache-hit job reads the same result as the job that parsed it | `tests/mcp_get_result.rs::cached_job_reads_the_same_result` |
| REQ-MCP-118 | a drawing without text has an empty label listing and still has images | `tests/mcp_get_result.rs::drawing_without_text_reads_cleanly` |
| REQ-MCP-119 | a page no larger than one tile has no tiles and tile_image points to page_image | `tests/mcp_get_result.rs::small_page_has_no_tiles` |
| REQ-MCP-120 | the library exposes the folder of a job's files for hosts that publish them | `tests/mcp_get_result.rs::service_exposes_the_result_folder` |

## get_job, list_jobs, delete_job

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-130 | get_job returns the job that the parse returned | `tests/mcp_jobs.rs::get_job_returns_the_parse_job` |
| REQ-MCP-131 | get_job with an unknown or malformed id is not_found | `tests/mcp_jobs.rs::get_job_unknown_id_is_not_found` |
| REQ-MCP-132 | list_jobs on an empty cache is an empty list at zero cost | `tests/mcp_jobs.rs::list_jobs_starts_empty` |
| REQ-MCP-133 | list_jobs returns the newest job first | `tests/mcp_jobs.rs::list_jobs_is_newest_first` |
| REQ-MCP-134 | list_jobs totals the cost: a fresh parse plus a cache hit is 1.1 | `tests/mcp_jobs.rs::list_jobs_totals_the_cost` |
| REQ-MCP-135 | list_jobs pages with limit and cursor, and the total covers all pages | `tests/mcp_jobs.rs::list_jobs_pages_with_a_cursor` |
| REQ-MCP-136 | list_jobs refuses a limit outside 1..100 and a foreign cursor | `tests/mcp_jobs.rs::list_jobs_checks_limit_and_cursor` |
| REQ-MCP-137 | delete_job removes the job and, when it was the last reader, the result | `tests/mcp_jobs.rs::delete_job_removes_job_and_result` |
| REQ-MCP-138 | delete_job keeps a result that another job still reads | `tests/mcp_jobs.rs::delete_job_keeps_a_shared_result` |
| REQ-MCP-139 | deleting the last reader of a shared result removes it | `tests/mcp_jobs.rs::deleting_every_reader_removes_the_result` |
| REQ-MCP-140 | delete_job on an unknown or already deleted job is not_found | `tests/mcp_jobs.rs::delete_job_twice_is_not_found` |
| REQ-MCP-141 | delete_job leaves the other jobs and their results alone | `tests/mcp_jobs.rs::delete_job_leaves_other_jobs` |
| REQ-MCP-142 | a failed job can be deleted; it has no result to remove | `tests/mcp_jobs.rs::failed_job_can_be_deleted` |

## parse_url and reset_url_cache

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-150 | parse_url downloads the drawing and parses it as a fresh job | `tests/mcp_parse_url.rs::parses_a_drawing_from_a_url` |
| REQ-MCP-151 | parse_url gives the same summary as parse_file of the same drawing | `tests/mcp_parse_url.rs::url_and_file_give_the_same_summary` |
| REQ-MCP-152 | a repeated parse_url is served from both caches at a tenth of the cost | `tests/mcp_parse_url.rs::repeated_url_is_cached_and_cheaper` |
| REQ-MCP-153 | a cached download makes no second HTTP request | `tests/mcp_parse_url.rs::cached_download_makes_no_request` |
| REQ-MCP-154 | the download cache is used even when the options differ | `tests/mcp_parse_url.rs::download_is_reused_for_other_options` |
| REQ-MCP-155 | a drawing parsed from a URL is a cache hit when parsed from a file | `tests/mcp_parse_url.rs::result_cache_is_shared_between_url_and_file` |
| REQ-MCP-156 | the file name comes from the URL path, percent-decoded | `tests/mcp_parse_url.rs::file_name_comes_from_the_url_path` |
| REQ-MCP-157 | Content-Disposition names the file when the URL does not | `tests/mcp_parse_url.rs::file_name_comes_from_content_disposition` |
| REQ-MCP-158 | redirects are followed and the final URL names the file | `tests/mcp_parse_url.rs::redirects_are_followed` |
| REQ-MCP-159 | an HTTP error status is download_failed and names the status | `tests/mcp_parse_url.rs::http_errors_are_download_failed` |
| REQ-MCP-160 | a host that does not answer is download_failed | `tests/mcp_parse_url.rs::unreachable_host_is_download_failed` |
| REQ-MCP-161 | a web page is refused as unsupported_file | `tests/mcp_parse_url.rs::web_page_is_unsupported` |
| REQ-MCP-162 | a file that is not named .dwg or .dxf is refused | `tests/mcp_parse_url.rs::other_file_type_is_unsupported` |
| REQ-MCP-163 | a download whose content is not a drawing is refused and not cached | `tests/mcp_parse_url.rs::non_dwg_download_is_refused_and_not_cached` |
| REQ-MCP-164 | a download announced as larger than 100 MB is refused as too_large | `tests/mcp_parse_url.rs::oversized_download_is_too_large` |
| REQ-MCP-165 | only http and https URLs with a host are accepted | `tests/mcp_parse_url.rs::url_scheme_is_checked` |
| REQ-MCP-166 | parse_url checks size before it downloads anything | `tests/mcp_parse_url.rs::options_are_checked_before_the_download` |
| REQ-MCP-167 | URLs that differ in their query string are separate downloads of one result | `tests/mcp_parse_url.rs::query_string_is_part_of_the_download_key` |
| REQ-MCP-168 | a download is still cached on its sixth day | `tests/mcp_parse_url.rs::download_is_cached_within_seven_days` |
| REQ-MCP-169 | after seven days the download is fetched again and parsed fresh | `tests/mcp_parse_url.rs::download_expires_after_seven_days` |
| REQ-MCP-170 | within seven days a changed remote file is not seen without a reset | `tests/mcp_parse_url.rs::changed_remote_file_is_not_seen_while_cached` |
| REQ-MCP-171 | reset_url_cache makes the next parse_url fetch and parse the changed file | `tests/mcp_parse_url.rs::reset_url_cache_picks_up_a_changed_file` |
| REQ-MCP-172 | reset_url_cache reports the download, results and jobs it removed | `tests/mcp_parse_url.rs::reset_url_cache_counts_what_it_removed` |
| REQ-MCP-173 | reset_url_cache is idempotent and makes no network request | `tests/mcp_parse_url.rs::reset_url_cache_twice_removes_nothing` |
| REQ-MCP-174 | reset_url_cache refuses a string that is not an http(s) URL | `tests/mcp_parse_url.rs::reset_url_cache_checks_the_url` |
| REQ-MCP-175 | reset_url_cache leaves other URLs cached | `tests/mcp_parse_url.rs::reset_url_cache_leaves_other_urls` |
| REQ-MCP-176 | reset_file_cache drops the result but keeps the download of the same drawing | `tests/mcp_parse_url.rs::reset_file_cache_keeps_the_download` |
| REQ-MCP-177 | the downloads and their jobs survive a server restart | `tests/mcp_parse_url.rs::download_cache_survives_a_restart` |

## DXF input

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-200 | parse_file parses a DXF into pages, labels and layers | `tests/mcp_dxf.rs::parse_file_reads_dxf` |
| REQ-MCP-201 | a DXF gives the same label texts and layers as the DWG of the same drawing | `tests/mcp_dxf.rs::dxf_and_dwg_give_the_same_labels` |
| REQ-MCP-202 | a DXF is cached like a DWG: the second parse costs a tenth | `tests/mcp_dxf.rs::dxf_is_cached` |
| REQ-MCP-203 | a parsed DXF is read with get_result like any drawing | `tests/mcp_dxf.rs::dxf_result_is_readable` |
| REQ-MCP-204 | parse_url downloads and parses a DXF | `tests/mcp_dxf.rs::parse_url_reads_dxf` |
| REQ-MCP-205 | a DXF that breaks off is a failed parse, not an empty drawing | `tests/mcp_dxf.rs::truncated_dxf_fails_to_parse` |
| REQ-MCP-206 | the format follows the content: a DXF named .dwg parses | `tests/mcp_dxf.rs::misnamed_dxf_parses` |
| REQ-MCP-207 | the tool definitions say that DXF is accepted | `tests/mcp_dxf.rs::definitions_mention_dxf` |
| REQ-MCP-208 | parse_file returns a single SVG page and its native labels | `tests/mcp_svg.rs::parse_file_reads_svg` |
| REQ-MCP-209 | parse_url reads and caches SVG drawings | `tests/mcp_svg.rs::parse_url_reads_svg` |
| REQ-MCP-210 | an SVG root with truncated XML fails without charging | `tests/mcp_svg.rs::truncated_svg_is_parse_failed` |
| REQ-MCP-211 | page_svg exposes a vector export and leaves page_image available for all inputs | `tests/mcp_svg.rs::vector_output_is_available_for_all_formats` |
| REQ-MCP-212 | remote MCP requires authentication and does not expose local tools | `tests/mcp_http.rs::http_authenticates_and_exposes_only_remote_tools` |

## Pure rules (unit tests)

| Id | Requirement | Test |
|----|-------------|------|
| REQ-MCP-U01 | timestamps are rendered as RFC 3339 UTC | `src/domain.rs::rfc3339_formats_known_instants` |
| REQ-MCP-U02 | a cache entry expires exactly seven days after it was created | `src/domain.rs::expiry_is_seven_days` |
| REQ-MCP-U03 | a cached parse costs one tenth of a fresh one | `src/domain.rs::cached_parse_is_ten_times_cheaper` |
| REQ-MCP-U04 | the file name of a download comes from the header, then the URL path | `src/domain.rs::download_name_prefers_content_disposition` |
| REQ-MCP-U05 | only http and https URLs with a host are accepted | `src/domain.rs::url_scheme_and_host_are_checked` |
| REQ-MCP-U06 | paging returns a cursor until the last page | `src/domain.rs::paginate_walks_all_items` |
| REQ-MCP-U07 | DWG, DXF and SVG are recognised by content and accepted by extension | `src/domain.rs::drawings_are_recognized` |
| REQ-MCP-U08 | a result key separates content, size and tiles | `src/domain.rs::result_key_depends_on_options` |
| REQ-MCP-U09 | a failsafe-recovered empty document is reported as unreadable | `src/domain.rs::failsafe_empty_document_is_unreadable` |
| REQ-MCP-U10 | a text DXF is complete only when it ends with EOF | `src/domain.rs::text_dxf_completeness` |
