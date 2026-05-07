#!/usr/bin/env bun

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Command, ValidationError } from "@cliffy/command";
import { type } from "arktype";
import { parseJSONC } from "confbox";
import { consola } from "consola";

const ALWAYS_REQUIRED_LANGUAGE_SERVERS: ReadonlyArray<string> = ["codebook", "harper-ls", "wakatime"];
const SKIPPED_LANGUAGES: ReadonlySet<string> = new Set(["JavaScript"]);

const isLanguageFormatterObject = type({
	"+": "reject",
	code_action: "string",
})
	.or({
		"+": "reject",
		external: {
			"+": "reject",
			"arguments?": "string[] | undefined | null",
			command: "string",
		},
	})
	.or({
		"+": "reject",
		language_server: {
			"+": "reject",
			name: "string",
		},
	});

const isLanguageFormatter = isLanguageFormatterObject
	.array()
	.or('"auto" | "none" | "prettier" | "language_server"')
	.or(isLanguageFormatterObject);

const isLanguageConfiguration = type({
	"+": "ignore",
	"formatter?": isLanguageFormatter.or("null | undefined"),
	"language_servers?": "string[] | null | undefined",
}).partial();

const isZedSettings = type({
	"+": "ignore",
	languages: type.Record("string", isLanguageConfiguration),
});

const command = new Command()
	.name("validate-settings")
	.description("Validate Zed editor settings")
	.version("1.0.0")
	.option("--validate-formatters", "Validate the formatters as well.")
	.argument("<file:file>", "The Zed settings.json file.", { default: resolve(".zed", "settings.json") })
	.action(async function onActionAsync({ validateFormatters }, file) {
		const value = isZedSettings(parseJSONC(await readFile(file, "utf8")));
		if (value instanceof type.errors) {
			throw new ValidationError(`The settings file at ${file} is invalid: ${value.summary}`);
		}

		for (const [language, configuration] of Object.entries(value.languages)) {
			if (SKIPPED_LANGUAGES.has(language)) continue;

			if (validateFormatters) {
				const { formatter } = configuration;
				if (typeof formatter !== "string") {
					
				}
			}
		}
	});

await command.parse(process.argv.slice(2));
