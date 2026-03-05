/** @type {import("lint-staged").Configuration} */
const configuration = {
	"*.md": ["rumdl check --fix", "rumdl fmt"],
	"*.rs": ["cargo fmt --"],
	"*.toml": ["tombi lint", "tombi format"],
};

export default configuration;
