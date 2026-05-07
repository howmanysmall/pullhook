## Correctness (239)

Code that is outright wrong or useless.

| Rule name                                           | Source     | Default | Enabled? | Fixable? |
| --------------------------------------------------- | ---------- | ------- | -------- | -------- |
| constructor-super                                   | eslint     | ✅      | ✅       |          |
| for-direction                                       | eslint     | ✅      | ✅       | ⚠️🛠️️       |
| getter-return                                       | eslint     | ✅      | ✅       |          |
| no-async-promise-executor                           | eslint     | ✅      | ✅       |          |
| no-caller                                           | eslint     | ✅      | ✅       |          |
| no-class-assign                                     | eslint     | ✅      | ✅       |          |
| no-compare-neg-zero                                 | eslint     | ✅      | ✅       | 🛠️💡      |
| no-cond-assign                                      | eslint     | ✅      | ✅       |          |
| no-const-assign                                     | eslint     | ✅      | ✅       |          |
| no-constant-binary-expression                       | eslint     | ✅      | ✅       |          |
| no-constant-condition                               | eslint     | ✅      | ✅       |          |
| no-control-regex                                    | eslint     | ✅      | ✅       |          |
| no-debugger                                         | eslint     | ✅      | ✅       | 🛠️        |
| no-delete-var                                       | eslint     | ✅      | ✅       |          |
| no-dupe-class-members                               | eslint     | ✅      | ✅       |          |
| no-dupe-else-if                                     | eslint     | ✅      | ✅       |          |
| no-dupe-keys                                        | eslint     | ✅      | ✅       |          |
| no-duplicate-case                                   | eslint     | ✅      | ✅       |          |
| no-empty-character-class                            | eslint     | ✅      | ✅       |          |
| no-empty-pattern                                    | eslint     | ✅      | ✅       |          |
| no-empty-static-block                               | eslint     | ✅      | ✅       | 💡       |
| no-eval                                             | eslint     | ✅      | ✅       |          |
| no-ex-assign                                        | eslint     | ✅      | ✅       |          |
| no-extra-boolean-cast                               | eslint     | ✅      | ✅       | 🛠️💡      |
| no-func-assign                                      | eslint     | ✅      | ✅       |          |
| no-global-assign                                    | eslint     | ✅      | ✅       |          |
| no-import-assign                                    | eslint     | ✅      | ✅       |          |
| no-invalid-regexp                                   | eslint     | ✅      | ✅       |          |
| no-irregular-whitespace                             | eslint     | ✅      | ✅       |          |
| no-iterator                                         | eslint     | ✅      | ✅       | 💡       |
| no-loss-of-precision                                | eslint     | ✅      | ✅       |          |
| no-misleading-character-class                       | eslint     | ✅      | ✅       | 🚧       |
| no-new-native-nonconstructor                        | eslint     | ✅      | ✅       |          |
| no-nonoctal-decimal-escape                          | eslint     | ✅      | ✅       | 💡       |
| no-obj-calls                                        | eslint     | ✅      | ✅       |          |
| no-self-assign                                      | eslint     | ✅      | ✅       |          |
| no-setter-return                                    | eslint     | ✅      | ✅       |          |
| no-shadow-restricted-names                          | eslint     | ✅      | ✅       |          |
| no-sparse-arrays                                    | eslint     | ✅      | ✅       |          |
| no-this-before-super                                | eslint     | ✅      | ✅       |          |
| no-unassigned-vars                                  | eslint     | ✅      | ✅       |          |
| no-unreachable                                      | eslint     | ✅      | ✅       |          |
| no-unsafe-finally                                   | eslint     | ✅      | ✅       |          |
| no-unsafe-negation                                  | eslint     | ✅      | ✅       | 🛠️        |
| no-unsafe-optional-chaining                         | eslint     | ✅      | ✅       |          |
| no-unused-expressions                               | eslint     | ✅      | ✅       |          |
| no-unused-labels                                    | eslint     | ✅      | ✅       | 🛠️        |
| no-unused-private-class-members                     | eslint     | ✅      | ✅       |          |
| no-unused-vars                                      | eslint     | ✅      | ✅       | ⚠️🛠️️💡     |
| no-useless-backreference                            | eslint     | ✅      | ✅       |          |
| no-useless-catch                                    | eslint     | ✅      | ✅       |          |
| no-useless-escape                                   | eslint     | ✅      | ✅       | 🛠️        |
| no-useless-rename                                   | eslint     | ✅      | ✅       | 🛠️        |
| no-with                                             | eslint     | ✅      | ✅       |          |
| require-yield                                       | eslint     | ✅      | ✅       |          |
| use-isnan                                           | eslint     | ✅      | ✅       | 🛠️        |
| valid-typeof                                        | eslint     | ✅      | ✅       | 🛠️        |
| default                                             | import     |         | ✅       |          |
| namespace                                           | import     |         | ✅       |          |
| expect-expect                                       | jest       |         |          |          |
| no-conditional-expect                               | jest       |         |          |          |
| no-disabled-tests                                   | jest       |         |          |          |
| no-export                                           | jest       |         |          |          |
| no-focused-tests                                    | jest       |         |          | 🛠️        |
| no-standalone-expect                                | jest       |         |          |          |
| prefer-snapshot-hint                                | jest       |         |          |          |
| require-to-throw-message                            | jest       |         |          |          |
| valid-describe-callback                             | jest       |         |          |          |
| valid-expect                                        | jest       |         |          | 💡       |
| valid-expect-in-promise                             | jest       |         |          |          |
| valid-title                                         | jest       |         |          | 🛠️        |
| check-property-names                                | jsdoc      |         | ✅       | 🚧       |
| check-tag-names                                     | jsdoc      |         | ✅       | 🚧       |
| implements-on-classes                               | jsdoc      |         | ✅       |          |
| no-defaults                                         | jsdoc      |         | ✅       | 🚧       |
| require-property                                    | jsdoc      |         | ✅       | 🚧       |
| require-property-description                        | jsdoc      |         | ✅       |          |
| require-property-name                               | jsdoc      |         | ✅       |          |
| require-property-type                               | jsdoc      |         | ✅       |          |
| require-yields                                      | jsdoc      |         | ✅       |          |
| alt-text                                            | jsx_a11y   |         |          |          |
| anchor-has-content                                  | jsx_a11y   |         |          | 💡       |
| anchor-is-valid                                     | jsx_a11y   |         |          |          |
| aria-activedescendant-has-tabindex                  | jsx_a11y   |         |          |          |
| aria-props                                          | jsx_a11y   |         |          | 🛠️        |
| aria-proptypes                                      | jsx_a11y   |         |          |          |
| aria-role                                           | jsx_a11y   |         |          |          |
| aria-unsupported-elements                           | jsx_a11y   |         |          | 🛠️        |
| autocomplete-valid                                  | jsx_a11y   |         |          |          |
| click-events-have-key-events                        | jsx_a11y   |         |          |          |
| heading-has-content                                 | jsx_a11y   |         |          |          |
| html-has-lang                                       | jsx_a11y   |         |          |          |
| iframe-has-title                                    | jsx_a11y   |         |          |          |
| img-redundant-alt                                   | jsx_a11y   |         |          |          |
| interactive-supports-focus                          | jsx_a11y   |         |          | 💡       |
| label-has-associated-control                        | jsx_a11y   |         |          |          |
| lang                                                | jsx_a11y   |         |          |          |
| media-has-caption                                   | jsx_a11y   |         |          |          |
| mouse-events-have-key-events                        | jsx_a11y   |         |          |          |
| no-access-key                                       | jsx_a11y   |         |          | 💡       |
| no-aria-hidden-on-focusable                         | jsx_a11y   |         |          | 🛠️        |
| no-autofocus                                        | jsx_a11y   |         |          | 💡       |
| no-distracting-elements                             | jsx_a11y   |         |          |          |
| no-noninteractive-tabindex                          | jsx_a11y   |         |          |          |
| no-redundant-roles                                  | jsx_a11y   |         |          | 🛠️        |
| no-static-element-interactions                      | jsx_a11y   |         |          |          |
| prefer-tag-over-role                                | jsx_a11y   |         |          |          |
| role-has-required-aria-props                        | jsx_a11y   |         |          |          |
| role-supports-aria-props                            | jsx_a11y   |         |          |          |
| scope                                               | jsx_a11y   |         |          | 🛠️        |
| tabindex-no-positive                                | jsx_a11y   |         |          | ⚠️💡      |
| google-font-display                                 | nextjs     |         |          |          |
| google-font-preconnect                              | nextjs     |         |          |          |
| inline-script-id                                    | nextjs     |         |          |          |
| next-script-for-ga                                  | nextjs     |         |          |          |
| no-assign-module-variable                           | nextjs     |         |          |          |
| no-async-client-component                           | nextjs     |         |          |          |
| no-before-interactive-script-outside-document       | nextjs     |         |          |          |
| no-css-tags                                         | nextjs     |         |          |          |
| no-document-import-in-page                          | nextjs     |         |          |          |
| no-duplicate-head                                   | nextjs     |         |          |          |
| no-head-element                                     | nextjs     |         |          |          |
| no-head-import-in-document                          | nextjs     |         |          |          |
| no-html-link-for-pages                              | nextjs     |         |          |          |
| no-img-element                                      | nextjs     |         |          | 🚧       |
| no-page-custom-font                                 | nextjs     |         |          |          |
| no-script-component-in-head                         | nextjs     |         |          |          |
| no-styled-jsx-in-document                           | nextjs     |         |          |          |
| no-sync-scripts                                     | nextjs     |         |          |          |
| no-title-in-document-head                           | nextjs     |         |          |          |
| no-typos                                            | nextjs     |         |          | 🚧       |
| no-unwanted-polyfillio                              | nextjs     |         |          |          |
| bad-array-method-on-arguments                       | oxc        | ✅      | ✅       |          |
| bad-char-at-comparison                              | oxc        | ✅      | ✅       |          |
| bad-comparison-sequence                             | oxc        | ✅      | ✅       |          |
| bad-min-max-func                                    | oxc        | ✅      | ✅       |          |
| bad-object-literal-comparison                       | oxc        | ✅      | ✅       |          |
| bad-replace-all-arg                                 | oxc        | ✅      | ✅       |          |
| const-comparisons                                   | oxc        | ✅      | ✅       |          |
| double-comparisons                                  | oxc        | ✅      | ✅       | 🛠️        |
| erasing-op                                          | oxc        | ✅      | ✅       | ⚠️🛠️️       |
| missing-throw                                       | oxc        | ✅      | ✅       | 💡       |
| number-arg-out-of-range                             | oxc        | ✅      | ✅       |          |
| only-used-in-recursion                              | oxc        | ✅      | ✅       | ⚠️🛠️️       |
| uninvoked-array-callback                            | oxc        | ✅      | ✅       |          |
| no-callback-in-promise                              | promise    |         | ✅       |          |
| no-new-statics                                      | promise    |         | ✅       | 🛠️        |
| valid-params                                        | promise    |         | ✅       |          |
| exhaustive-deps                                     | react      |         |          | ⚠️🛠️️💡     |
| forward-ref-uses-ref                                | react      |         |          | 💡       |
| jsx-key                                             | react      |         |          |          |
| jsx-no-duplicate-props                              | react      |         |          |          |
| jsx-no-undef                                        | react      |         |          |          |
| jsx-props-no-spread-multi                           | react      |         |          | 🛠️        |
| no-children-prop                                    | react      |         |          |          |
| no-danger-with-children                             | react      |         |          |          |
| no-did-mount-set-state                              | react      |         |          |          |
| no-did-update-set-state                             | react      |         |          |          |
| no-direct-mutation-state                            | react      |         |          |          |
| no-find-dom-node                                    | react      |         |          |          |
| no-is-mounted                                       | react      |         |          |          |
| no-render-return-value                              | react      |         |          |          |
| no-string-refs                                      | react      |         |          |          |
| no-this-in-sfc                                      | react      |         |          |          |
| no-unsafe                                           | react      |         |          |          |
| no-will-update-set-state                            | react      |         |          |          |
| void-dom-elements-no-children                       | react      |         |          |          |
| await-thenable                                      | typescript | ✅      | ✅       | 💡       |
| no-array-delete                                     | typescript | ✅      | ✅       | 💡       |
| no-base-to-string                                   | typescript | ✅      | ✅       |          |
| no-duplicate-enum-values                            | typescript | ✅      | ✅       |          |
| no-duplicate-type-constituents                      | typescript | ✅      | ✅       | 🛠️        |
| no-extra-non-null-assertion                         | typescript | ✅      | ✅       | 🛠️        |
| no-floating-promises                                | typescript | ✅      | ✅       | 💡       |
| no-for-in-array                                     | typescript | ✅      | ✅       |          |
| no-implied-eval                                     | typescript | ✅      | ✅       |          |
| no-meaningless-void-operator                        | typescript | ✅      | ✅       | 🛠️💡      |
| no-misused-new                                      | typescript | ✅      | ✅       |          |
| no-misused-spread                                   | typescript | ✅      | ✅       | 💡       |
| no-non-null-asserted-optional-chain                 | typescript | ✅      | ✅       | 💡       |
| no-redundant-type-constituents                      | typescript | ✅      | ✅       |          |
| no-this-alias                                       | typescript | ✅      | ✅       |          |
| no-unnecessary-parameter-property-assignment        | typescript | ✅      | ✅       | 💡       |
| no-unsafe-declaration-merging                       | typescript | ✅      | ✅       |          |
| no-unsafe-unary-minus                               | typescript | ✅      | ✅       |          |
| no-useless-default-assignment                       | typescript | ✅      | ✅       |          |
| no-useless-empty-export                             | typescript | ✅      | ✅       | 🛠️        |
| no-wrapper-object-types                             | typescript | ✅      | ✅       | 🛠️        |
| prefer-as-const                                     | typescript | ✅      | ✅       | 🛠️        |
| prefer-namespace-keyword                            | typescript | ✅      | ✅       | 🛠️        |
| require-array-sort-compare                          | typescript | ✅      | ✅       |          |
| restrict-template-expressions                       | typescript | ✅      | ✅       |          |
| triple-slash-reference                              | typescript | ✅      | ✅       |          |
| unbound-method                                      | typescript | ✅      | ✅       |          |
| no-await-in-promise-methods                         | unicorn    | ✅      | ✅       | 💡       |
| no-empty-file                                       | unicorn    | ✅      | ✅       |          |
| no-invalid-fetch-options                            | unicorn    | ✅      | ✅       |          |
| no-invalid-remove-event-listener                    | unicorn    | ✅      | ✅       |          |
| no-new-array                                        | unicorn    | ✅      | ✅       | 🚧       |
| no-single-promise-in-promise-methods                | unicorn    | ✅      | ✅       | 🛠️        |
| no-thenable                                         | unicorn    | ✅      | ✅       |          |
| no-unnecessary-await                                | unicorn    | ✅      | ✅       | 🛠️        |
| no-useless-fallback-in-spread                       | unicorn    | ✅      | ✅       | 🛠️        |
| no-useless-length-check                             | unicorn    | ✅      | ✅       | 🚧       |
| no-useless-spread                                   | unicorn    | ✅      | ✅       | ⚠️🛠️️       |
| prefer-set-size                                     | unicorn    | ✅      | ✅       | 🛠️        |
| prefer-string-starts-ends-with                      | unicorn    | ✅      | ✅       | 🛠️        |
| consistent-each-for                                 | vitest     |         |          |          |
| expect-expect                                       | vitest     |         |          |          |
| hoisted-apis-on-top                                 | vitest     |         |          | 💡       |
| no-conditional-expect                               | vitest     |         |          |          |
| no-conditional-tests                                | vitest     |         |          |          |
| no-disabled-tests                                   | vitest     |         |          |          |
| no-focused-tests                                    | vitest     |         |          | 🛠️        |
| no-standalone-expect                                | vitest     |         |          |          |
| prefer-snapshot-hint                                | vitest     |         |          |          |
| require-awaited-expect-poll                         | vitest     |         |          |          |
| require-local-test-context-for-concurrent-snapshots | vitest     |         |          |          |
| require-mock-type-parameters                        | vitest     |         |          |          |
| require-to-throw-message                            | vitest     |         |          |          |
| valid-describe-callback                             | vitest     |         |          |          |
| valid-expect                                        | vitest     |         |          | 💡       |
| valid-expect-in-promise                             | vitest     |         |          |          |
| valid-title                                         | vitest     |         |          | 🛠️        |
| warn-todo                                           | vitest     |         |          |          |
| no-arrow-functions-in-watch                         | vue        |         |          |          |
| no-deprecated-data-object-declaration               | vue        |         |          | 🚧       |
| no-deprecated-delete-set                            | vue        |         |          |          |
| no-deprecated-destroyed-lifecycle                   | vue        |         |          | 🛠️        |
| no-deprecated-events-api                            | vue        |         |          |          |
| no-deprecated-model-definition                      | vue        |         |          | 🚧       |
| no-deprecated-vue-config-keycodes                   | vue        |         |          |          |
| no-export-in-script-setup                           | vue        |         |          |          |
| no-lifecycle-after-await                            | vue        |         |          |          |
| no-this-in-before-route-enter                       | vue        |         |          |          |
| prefer-import-from-vue                              | vue        |         |          | 🛠️        |
| return-in-computed-property                         | vue        |         |          |          |
| valid-define-emits                                  | vue        |         |          | 🚧       |
| valid-define-props                                  | vue        |         |          | 🚧       |

## Perf (13)

Code that can be written to run faster.

| Rule name                         | Source     | Default | Enabled? | Fixable? |
| --------------------------------- | ---------- | ------- | -------- | -------- |
| no-await-in-loop                  | eslint     |         | ✅       |          |
| no-useless-call                   | eslint     |         | ✅       |          |
| no-accumulating-spread            | oxc        |         | ✅       |          |
| no-map-spread                     | oxc        |         | ✅       | 🛠️💡      |
| jsx-no-constructed-context-values | react      |         |          |          |
| no-array-index-key                | react      |         |          |          |
| jsx-no-jsx-as-prop                | react_perf |         |          |          |
| jsx-no-new-array-as-prop          | react_perf |         |          |          |
| jsx-no-new-function-as-prop       | react_perf |         |          |          |
| jsx-no-new-object-as-prop         | react_perf |         |          |          |
| prefer-array-find                 | unicorn    |         | ✅       | 🚧       |
| prefer-array-flat-map             | unicorn    |         | ✅       | 🛠️        |
| prefer-set-has                    | unicorn    |         | ✅       | ⚠️🛠️️       |

## Restriction (94)

Lints which prevent the use of language and library features. Must not be enabled as a whole, should be considered on a case-by-case basis before enabling.

| Rule name                               | Source     | Default | Enabled? | Fixable? |
| --------------------------------------- | ---------- | ------- | -------- | -------- |
| class-methods-use-this                  | eslint     |         | ✅       |          |
| complexity                              | eslint     |         | ✅       |          |
| default-case                            | eslint     |         | ✅       |          |
| no-alert                                | eslint     |         | ✅       |          |
| no-bitwise                              | eslint     |         | ✅       |          |
| no-console                              | eslint     |         | ✅       | 💡       |
| no-div-regex                            | eslint     |         | ✅       | 🛠️        |
| no-empty                                | eslint     |         | ✅       | 💡       |
| no-empty-function                       | eslint     |         | ✅       | 💡       |
| no-eq-null                              | eslint     |         | ✅       | ⚠️🛠️️       |
| no-param-reassign                       | eslint     |         | ✅       |          |
| no-plusplus                             | eslint     |         | ✅       | 💡       |
| no-proto                                | eslint     |         | ✅       | 🚧       |
| no-regex-spaces                         | eslint     |         | ✅       | 🛠️        |
| no-restricted-globals                   | eslint     |         | ✅       |          |
| no-restricted-imports                   | eslint     |         | ✅       |          |
| no-restricted-properties                | eslint     |         | ✅       |          |
| no-sequences                            | eslint     |         | ✅       |          |
| no-undefined                            | eslint     |         | ✅       |          |
| no-use-before-define                    | eslint     |         | ✅       |          |
| no-var                                  | eslint     |         | ✅       | 🛠️        |
| no-void                                 | eslint     |         | ✅       | 💡       |
| unicode-bom                             | eslint     |         | ✅       | 🛠️        |
| extensions                              | import     |         | ✅       |          |
| no-amd                                  | import     |         | ✅       |          |
| no-commonjs                             | import     |         | ✅       |          |
| no-cycle                                | import     |         | ✅       |          |
| no-default-export                       | import     |         | ✅       |          |
| no-dynamic-require                      | import     |         | ✅       |          |
| no-relative-parent-imports              | import     |         | ✅       |          |
| no-webpack-loader-syntax                | import     |         | ✅       |          |
| unambiguous                             | import     |         | ✅       |          |
| check-access                            | jsdoc      |         | ✅       |          |
| empty-tags                              | jsdoc      |         | ✅       | 🚧       |
| anchor-ambiguous-text                   | jsx_a11y   |         |          |          |
| handle-callback-err                     | node       |         | ✅       |          |
| no-new-require                          | node       |         | ✅       |          |
| no-path-concat                          | node       |         | ✅       |          |
| no-process-env                          | node       |         | ✅       |          |
| bad-bitwise-operator                    | oxc        |         | ✅       | 💡       |
| no-async-await                          | oxc        |         | ✅       |          |
| no-barrel-file                          | oxc        |         | ✅       |          |
| no-const-enum                           | oxc        |         | ✅       | 🛠️        |
| no-optional-chaining                    | oxc        |         | ✅       |          |
| no-rest-spread-properties               | oxc        |         | ✅       |          |
| catch-or-return                         | promise    |         | ✅       |          |
| spec-only                               | promise    |         | ✅       |          |
| button-has-type                         | react      |         |          |          |
| forbid-component-props                  | react      |         |          |          |
| forbid-dom-props                        | react      |         |          |          |
| forbid-elements                         | react      |         |          |          |
| jsx-filename-extension                  | react      |         |          | 🚧       |
| no-clone-element                        | react      |         |          |          |
| no-danger                               | react      |         |          |          |
| no-multi-comp                           | react      |         |          |          |
| no-react-children                       | react      |         |          |          |
| no-unknown-property                     | react      |         |          | 🚧       |
| only-export-components                  | react      |         |          |          |
| prefer-function-component               | react      |         |          |          |
| explicit-function-return-type           | typescript |         | ✅       |          |
| explicit-member-accessibility           | typescript |         | ✅       | 🛠️💡      |
| explicit-module-boundary-types          | typescript |         | ✅       |          |
| no-dynamic-delete                       | typescript |         | ✅       |          |
| no-empty-object-type                    | typescript |         | ✅       | 🚧       |
| no-explicit-any                         | typescript |         | ✅       | 🛠️        |
| no-import-type-side-effects             | typescript |         | ✅       | 🛠️        |
| no-invalid-void-type                    | typescript |         | ✅       |          |
| no-namespace                            | typescript |         | ✅       |          |
| no-non-null-asserted-nullish-coalescing | typescript |         | ✅       | 💡       |
| no-non-null-assertion                   | typescript |         | ✅       | 🚧       |
| no-require-imports                      | typescript |         | ✅       | 🚧       |
| no-restricted-types                     | typescript |         | ✅       | 🛠️💡      |
| no-var-requires                         | typescript |         | ✅       |          |
| non-nullable-type-assertion-style       | typescript |         | ✅       | 🛠️        |
| prefer-literal-enum-member              | typescript |         | ✅       |          |
| promise-function-async                  | typescript |         | ✅       | 🛠️        |
| use-unknown-in-catch-callback-variable  | typescript |         | ✅       | 💡       |
| no-abusive-eslint-disable               | unicorn    |         | ✅       |          |
| no-anonymous-default-export             | unicorn    |         | ✅       | 🚧       |
| no-array-for-each                       | unicorn    |         | ✅       | 🚧       |
| no-array-reduce                         | unicorn    |         | ✅       |          |
| no-document-cookie                      | unicorn    |         | ✅       |          |
| no-length-as-slice-end                  | unicorn    |         | ✅       | 🛠️        |
| no-magic-array-flat-depth               | unicorn    |         | ✅       |          |
| no-process-exit                         | unicorn    |         | ✅       | 🚧       |
| no-useless-error-capture-stack-trace    | unicorn    |         | ✅       | 💡       |
| prefer-modern-math-apis                 | unicorn    |         | ✅       | 🚧       |
| prefer-module                           | unicorn    |         | ✅       | 🚧       |
| prefer-node-protocol                    | unicorn    |         | ✅       | 🛠️        |
| prefer-number-properties                | unicorn    |         | ✅       | ⚠️🛠️️       |
| require-test-timeout                    | vitest     |         |          |          |
| max-props                               | vue        |         |          |          |
| no-import-compiler-macros               | vue        |         |          | ⚠️🛠️️       |
| no-multiple-slot-args                   | vue        |         |          | 🚧       |

## Suspicious (55)

code that is most likely wrong or useless.

| Rule name                              | Source     | Default | Enabled? | Fixable? |
| -------------------------------------- | ---------- | ------- | -------- | -------- |
| block-scoped-var                       | eslint     |         | ✅       |          |
| no-extend-native                       | eslint     |         | ✅       |          |
| no-extra-bind                          | eslint     |         | ✅       | 🚧       |
| no-new                                 | eslint     |         | ✅       |          |
| no-shadow                              | eslint     |         | ✅       |          |
| no-underscore-dangle                   | eslint     |         | ✅       |          |
| no-unexpected-multiline                | eslint     |         | ✅       | ⚠️🛠️️       |
| no-unmodified-loop-condition           | eslint     |         | ✅       |          |
| no-unneeded-ternary                    | eslint     |         | ✅       | ⚠️🛠️️       |
| no-useless-concat                      | eslint     |         | ✅       | 🚧       |
| no-useless-constructor                 | eslint     |         | ✅       | 💡       |
| preserve-caught-error                  | eslint     |         | ✅       | 🛠️        |
| no-absolute-path                       | import     |         | ✅       | 🚧       |
| no-empty-named-blocks                  | import     |         | ✅       | 🛠️        |
| no-named-as-default                    | import     |         | ✅       |          |
| no-named-as-default-member             | import     |         | ✅       |          |
| no-self-import                         | import     |         | ✅       |          |
| no-unassigned-import                   | import     |         | ✅       |          |
| no-commented-out-tests                 | jest       |         |          |          |
| approx-constant                        | oxc        |         | ✅       | 💡       |
| misrefactored-assign-op                | oxc        |         | ✅       | 💡       |
| no-async-endpoint-handlers             | oxc        |         | ✅       |          |
| no-this-in-exported-function           | oxc        |         | ✅       |          |
| always-return                          | promise    |         | ✅       |          |
| no-multiple-resolved                   | promise    |         | ✅       |          |
| no-promise-in-callback                 | promise    |         | ✅       |          |
| iframe-missing-sandbox                 | react      |         |          | 🚧       |
| jsx-no-comment-textnodes               | react      |         |          |          |
| jsx-no-script-url                      | react      |         |          | 🚧       |
| no-namespace                           | react      |         | ✅       |          |
| react-in-jsx-scope                     | react      |         |          |          |
| style-prop-object                      | react      |         |          |          |
| consistent-return                      | typescript |         | ✅       |          |
| no-confusing-non-null-assertion        | typescript |         | ✅       | 🚧       |
| no-extraneous-class                    | typescript |         | ✅       | ⚠️💡      |
| no-unnecessary-boolean-literal-compare | typescript |         | ✅       | 🚧       |
| no-unnecessary-template-expression     | typescript |         | ✅       | 🛠️        |
| no-unnecessary-type-arguments          | typescript |         | ✅       | 🛠️        |
| no-unnecessary-type-assertion          | typescript |         | ✅       | 🛠️        |
| no-unnecessary-type-constraint         | typescript |         | ✅       | 🚧       |
| no-unnecessary-type-conversion         | typescript |         | ✅       |          |
| no-unnecessary-type-parameters         | typescript |         | ✅       |          |
| no-unsafe-enum-comparison              | typescript |         | ✅       | 💡       |
| no-unsafe-type-assertion               | typescript |         | ✅       |          |
| consistent-function-scoping            | unicorn    |         | ✅       | 🚧       |
| no-accessor-recursion                  | unicorn    |         | ✅       |          |
| no-array-reverse                       | unicorn    |         | ✅       | 🛠️        |
| no-array-sort                          | unicorn    |         | ✅       | 🛠️        |
| no-instanceof-builtins                 | unicorn    |         | ✅       | 💡       |
| prefer-add-event-listener              | unicorn    |         | ✅       | 🚧       |
| require-module-specifiers              | unicorn    |         | ✅       | 🛠️        |
| require-post-message-target-origin     | unicorn    |         | ✅       | 💡       |
| no-commented-out-tests                 | vitest     |         |          |          |
| no-required-prop-with-default          | vue        |         |          | 💡       |
| require-default-export                 | vue        |         |          |          |

## Pedantic (121)

Lints which are rather strict or have occasional false positives.

| Rule name                               | Source     | Default | Enabled? | Fixable? |
| --------------------------------------- | ---------- | ------- | -------- | -------- |
| accessor-pairs                          | eslint     |         | ✅       |          |
| array-callback-return                   | eslint     |         | ✅       | 🚧       |
| eqeqeq                                  | eslint     |         | ✅       | ⚠️🛠️️       |
| max-classes-per-file                    | eslint     |         | ✅       |          |
| max-depth                               | eslint     |         | ✅       |          |
| max-lines                               | eslint     |         | ✅       |          |
| max-lines-per-function                  | eslint     |         | ✅       |          |
| max-nested-callbacks                    | eslint     |         | ✅       |          |
| no-array-constructor                    | eslint     |         | ✅       | 🛠️        |
| no-case-declarations                    | eslint     |         | ✅       | 💡       |
| no-constructor-return                   | eslint     |         | ✅       |          |
| no-else-return                          | eslint     |         | ✅       | 🛠️        |
| no-fallthrough                          | eslint     |         | ✅       | 🚧       |
| no-inline-comments                      | eslint     |         | ✅       |          |
| no-inner-declarations                   | eslint     |         | ✅       |          |
| no-lonely-if                            | eslint     |         | ✅       | 🚧       |
| no-loop-func                            | eslint     |         | ✅       |          |
| no-negated-condition                    | eslint     |         | ✅       | 🚧       |
| no-new-wrappers                         | eslint     |         | ✅       | 🛠️        |
| no-object-constructor                   | eslint     |         | ✅       | 🚧       |
| no-promise-executor-return              | eslint     |         | ✅       | 🚧       |
| no-prototype-builtins                   | eslint     |         | ✅       | 🚧       |
| no-redeclare                            | eslint     |         | ✅       |          |
| no-self-compare                         | eslint     |         | ✅       |          |
| no-throw-literal                        | eslint     |         | ✅       | 💡       |
| no-useless-return                       | eslint     |         | ✅       | 🚧       |
| no-warning-comments                     | eslint     |         | ✅       |          |
| radix                                   | eslint     |         | ✅       | ⚠️🛠️️       |
| require-await                           | eslint     |         | ✅       | ⚠️🛠️️       |
| require-unicode-regexp                  | eslint     |         | ✅       | 🚧       |
| sort-vars                               | eslint     |         | ✅       | 🚧       |
| symbol-description                      | eslint     |         | ✅       |          |
| max-dependencies                        | import     |         | ✅       |          |
| no-conditional-in-test                  | jest       |         |          |          |
| require-param                           | jsdoc      |         | ✅       | 🚧       |
| require-param-description               | jsdoc      |         | ✅       | 🚧       |
| require-param-name                      | jsdoc      |         | ✅       |          |
| require-param-type                      | jsdoc      |         | ✅       | 🚧       |
| require-returns                         | jsdoc      |         | ✅       | 🚧       |
| require-returns-description             | jsdoc      |         | ✅       |          |
| require-returns-type                    | jsdoc      |         | ✅       |          |
| branches-sharing-code                   | oxc        |         | ✅       | 💡       |
| checked-requires-onchange-or-readonly   | react      |         |          |          |
| display-name                            | react      |         |          |          |
| jsx-no-target-blank                     | react      |         |          | 🚧       |
| jsx-no-useless-fragment                 | react      |         |          | 💡       |
| no-unescaped-entities                   | react      |         |          | 🚧       |
| rules-of-hooks                          | react      |         |          |          |
| ban-ts-comment                          | typescript |         | ✅       | 🛠️        |
| ban-types                               | typescript |         | ✅       |          |
| no-confusing-void-expression            | typescript |         | ✅       | 🛠️💡      |
| no-deprecated                           | typescript |         | ✅       |          |
| no-misused-promises                     | typescript |         | ✅       |          |
| no-mixed-enums                          | typescript |         | ✅       |          |
| no-unsafe-argument                      | typescript |         | ✅       |          |
| no-unsafe-assignment                    | typescript |         | ✅       |          |
| no-unsafe-call                          | typescript |         | ✅       |          |
| no-unsafe-function-type                 | typescript |         | ✅       |          |
| no-unsafe-member-access                 | typescript |         | ✅       |          |
| no-unsafe-return                        | typescript |         | ✅       |          |
| only-throw-error                        | typescript |         | ✅       |          |
| prefer-enum-initializers                | typescript |         | ✅       | 💡       |
| prefer-includes                         | typescript |         | ✅       | 🛠️        |
| prefer-nullish-coalescing               | typescript |         | ✅       | 🛠️        |
| prefer-promise-reject-errors            | typescript |         | ✅       |          |
| prefer-readonly-parameter-types         | typescript |         | ✅       |          |
| prefer-ts-expect-error                  | typescript |         | ✅       | 🛠️        |
| related-getter-setter-pairs             | typescript |         | ✅       |          |
| require-await                           | typescript |         | ✅       | 💡       |
| restrict-plus-operands                  | typescript |         | ✅       |          |
| return-await                            | typescript |         | ✅       | 🛠️💡      |
| strict-boolean-expressions              | typescript |         | ✅       | 🚧       |
| strict-void-return                      | typescript |         | ✅       |          |
| switch-exhaustiveness-check             | typescript |         | ✅       | 💡       |
| consistent-assert                       | unicorn    |         | ✅       | 🛠️        |
| consistent-empty-array-spread           | unicorn    |         | ✅       | 💡       |
| escape-case                             | unicorn    |         | ✅       | 🛠️        |
| explicit-length-check                   | unicorn    |         | ✅       | 🛠️        |
| new-for-builtins                        | unicorn    |         | ✅       | 🚧       |
| no-array-callback-reference             | unicorn    |         | ✅       | 🚧       |
| no-hex-escape                           | unicorn    |         | ✅       | 🛠️        |
| no-immediate-mutation                   | unicorn    |         | ✅       | 🚧       |
| no-instanceof-array                     | unicorn    |         | ✅       | 🛠️        |
| no-lonely-if                            | unicorn    |         | ✅       | 🚧       |
| no-negated-condition                    | unicorn    |         | ✅       | 🚧       |
| no-negation-in-equality-check           | unicorn    |         | ✅       | 💡       |
| no-new-buffer                           | unicorn    |         | ✅       | 💡       |
| no-object-as-default-parameter          | unicorn    |         | ✅       |          |
| no-static-only-class                    | unicorn    |         | ✅       | ⚠️🛠️️       |
| no-this-assignment                      | unicorn    |         | ✅       |          |
| no-typeof-undefined                     | unicorn    |         | ✅       | 🛠️💡      |
| no-unnecessary-array-flat-depth         | unicorn    |         | ✅       | 💡       |
| no-unnecessary-array-splice-count       | unicorn    |         | ✅       | 🛠️        |
| no-unnecessary-slice-end                | unicorn    |         | ✅       | 🛠️        |
| no-unreadable-iife                      | unicorn    |         | ✅       |          |
| no-useless-promise-resolve-reject       | unicorn    |         | ✅       | 🛠️        |
| no-useless-switch-case                  | unicorn    |         | ✅       | 🚧       |
| no-useless-undefined                    | unicorn    |         | ✅       | 🛠️        |
| prefer-array-flat                       | unicorn    |         | ✅       | ⚠️🛠️️       |
| prefer-array-some                       | unicorn    |         | ✅       | 💡       |
| prefer-at                               | unicorn    |         | ✅       | ⚠️🛠️️       |
| prefer-blob-reading-methods             | unicorn    |         | ✅       | 🚧       |
| prefer-code-point                       | unicorn    |         | ✅       | 🛠️        |
| prefer-date-now                         | unicorn    |         | ✅       | 🛠️        |
| prefer-dom-node-append                  | unicorn    |         | ✅       | 🛠️        |
| prefer-dom-node-dataset                 | unicorn    |         | ✅       | 🛠️        |
| prefer-dom-node-remove                  | unicorn    |         | ✅       | 🚧       |
| prefer-event-target                     | unicorn    |         | ✅       |          |
| prefer-import-meta-properties           | unicorn    |         | ✅       | 🛠️        |
| prefer-math-min-max                     | unicorn    |         | ✅       | 🛠️        |
| prefer-math-trunc                       | unicorn    |         | ✅       | 💡       |
| prefer-native-coercion-functions        | unicorn    |         | ✅       | 🚧       |
| prefer-prototype-methods                | unicorn    |         | ✅       | 🛠️        |
| prefer-query-selector                   | unicorn    |         | ✅       | 🛠️        |
| prefer-regexp-test                      | unicorn    |         | ✅       | 🛠️        |
| prefer-string-replace-all               | unicorn    |         | ✅       | 🛠️        |
| prefer-string-slice                     | unicorn    |         | ✅       | 🛠️        |
| prefer-top-level-await                  | unicorn    |         | ✅       | 🚧       |
| prefer-type-error                       | unicorn    |         | ✅       | 🛠️        |
| require-number-to-fixed-digits-argument | unicorn    |         | ✅       | 🛠️        |
| no-conditional-in-test                  | vitest     |         |          |          |

## Style (253)

Code that should be written in a more idiomatic way.

| Rule name                            | Source     | Default | Enabled? | Fixable? |
| ------------------------------------ | ---------- | ------- | -------- | -------- |
| arrow-body-style                     | eslint     |         | ✅       | 🛠️        |
| capitalized-comments                 | eslint     |         | ✅       | 🛠️        |
| curly                                | eslint     |         | ✅       | 🛠️        |
| default-case-last                    | eslint     |         | ✅       |          |
| default-param-last                   | eslint     |         | ✅       |          |
| func-name-matching                   | eslint     |         | ✅       |          |
| func-names                           | eslint     |         | ✅       | 🛠️💡      |
| func-style                           | eslint     |         | ✅       | 🚧       |
| grouped-accessor-pairs               | eslint     |         | ✅       | 🚧       |
| guard-for-in                         | eslint     |         | ✅       |          |
| id-length                            | eslint     |         | ✅       |          |
| init-declarations                    | eslint     |         | ✅       |          |
| logical-assignment-operators         | eslint     |         | ✅       | 🚧       |
| max-params                           | eslint     |         | ✅       |          |
| max-statements                       | eslint     |         | ✅       |          |
| new-cap                              | eslint     |         | ✅       | 🚧       |
| no-continue                          | eslint     |         | ✅       |          |
| no-duplicate-imports                 | eslint     |         | ✅       | 🚧       |
| no-extra-label                       | eslint     |         | ✅       | 🛠️        |
| no-implicit-coercion                 | eslint     |         | ✅       | 🛠️        |
| no-label-var                         | eslint     |         | ✅       |          |
| no-labels                            | eslint     |         | ✅       |          |
| no-lone-blocks                       | eslint     |         | ✅       |          |
| no-magic-numbers                     | eslint     |         | ✅       | 🚧       |
| no-multi-assign                      | eslint     |         | ✅       |          |
| no-multi-str                         | eslint     |         | ✅       |          |
| no-nested-ternary                    | eslint     |         | ✅       |          |
| no-new-func                          | eslint     |         | ✅       |          |
| no-return-assign                     | eslint     |         | ✅       |          |
| no-script-url                        | eslint     |         | ✅       |          |
| no-template-curly-in-string          | eslint     |         | ✅       |          |
| no-ternary                           | eslint     |         | ✅       |          |
| no-useless-computed-key              | eslint     |         | ✅       | 🛠️        |
| object-shorthand                     | eslint     |         | ✅       | 🛠️        |
| operator-assignment                  | eslint     |         | ✅       | ⚠️🛠️️       |
| prefer-const                         | eslint     |         | ✅       | 🛠️        |
| prefer-destructuring                 | eslint     |         | ✅       | 🛠️        |
| prefer-exponentiation-operator       | eslint     |         | ✅       | 🛠️        |
| prefer-numeric-literals              | eslint     |         | ✅       | 🛠️        |
| prefer-object-has-own                | eslint     |         | ✅       | 🛠️        |
| prefer-object-spread                 | eslint     |         | ✅       | 🛠️        |
| prefer-promise-reject-errors         | eslint     |         | ✅       |          |
| prefer-rest-params                   | eslint     |         | ✅       |          |
| prefer-spread                        | eslint     |         | ✅       |          |
| prefer-template                      | eslint     |         | ✅       | 🛠️        |
| sort-imports                         | eslint     |         | ✅       | 🛠️        |
| sort-keys                            | eslint     |         | ✅       | 🛠️        |
| vars-on-top                          | eslint     |         | ✅       |          |
| yoda                                 | eslint     |         | ✅       | 🛠️        |
| consistent-type-specifier-style      | import     |         | ✅       | 🛠️        |
| exports-last                         | import     |         | ✅       |          |
| first                                | import     |         | ✅       | 🚧       |
| group-exports                        | import     |         | ✅       |          |
| no-anonymous-default-export          | import     |         | ✅       |          |
| no-duplicates                        | import     |         | ✅       |          |
| no-mutable-exports                   | import     |         | ✅       |          |
| no-named-default                     | import     |         | ✅       |          |
| no-named-export                      | import     |         | ✅       |          |
| no-namespace                         | import     |         | ✅       | 🚧       |
| no-nodejs-modules                    | import     |         | ✅       |          |
| prefer-default-export                | import     |         | ✅       |          |
| consistent-test-it                   | jest       |         |          | 🛠️        |
| max-expects                          | jest       |         |          |          |
| max-nested-describe                  | jest       |         |          |          |
| no-alias-methods                     | jest       |         |          | 🛠️        |
| no-confusing-set-timeout             | jest       |         |          |          |
| no-deprecated-functions              | jest       |         |          | 🛠️        |
| no-done-callback                     | jest       |         |          | 🚧       |
| no-duplicate-hooks                   | jest       |         |          |          |
| no-hooks                             | jest       |         |          |          |
| no-identical-title                   | jest       |         |          |          |
| no-interpolation-in-snapshots        | jest       |         |          |          |
| no-jasmine-globals                   | jest       |         |          | 🛠️        |
| no-large-snapshots                   | jest       |         |          |          |
| no-mocks-import                      | jest       |         |          |          |
| no-restricted-jest-methods           | jest       |         |          |          |
| no-restricted-matchers               | jest       |         |          |          |
| no-test-prefixes                     | jest       |         |          | 🛠️        |
| no-test-return-statement             | jest       |         |          |          |
| no-unneeded-async-expect-function    | jest       |         |          | 🛠️        |
| no-untyped-mock-factory              | jest       |         |          | 🛠️        |
| padding-around-after-all-blocks      | jest       |         |          | 🛠️        |
| padding-around-test-blocks           | jest       |         |          | 🛠️        |
| prefer-called-with                   | jest       |         |          | 🛠️        |
| prefer-comparison-matcher            | jest       |         |          | 🛠️        |
| prefer-each                          | jest       |         |          |          |
| prefer-ending-with-an-expect         | jest       |         |          |          |
| prefer-equality-matcher              | jest       |         |          | 💡       |
| prefer-expect-assertions             | jest       |         |          | 💡       |
| prefer-expect-resolves               | jest       |         |          | 🛠️        |
| prefer-hooks-in-order                | jest       |         |          |          |
| prefer-hooks-on-top                  | jest       |         |          |          |
| prefer-importing-jest-globals        | jest       |         |          | 🛠️        |
| prefer-jest-mocked                   | jest       |         |          | 🛠️        |
| prefer-lowercase-title               | jest       |         |          | 🛠️        |
| prefer-mock-promise-shorthand        | jest       |         |          | 🛠️        |
| prefer-mock-return-shorthand         | jest       |         |          | 🛠️        |
| prefer-spy-on                        | jest       |         |          | 💡       |
| prefer-strict-equal                  | jest       |         |          | 🛠️        |
| prefer-to-be                         | jest       |         |          | 🛠️        |
| prefer-to-contain                    | jest       |         |          | 🛠️        |
| prefer-to-have-been-called           | jest       |         |          | 🛠️        |
| prefer-to-have-been-called-times     | jest       |         |          | 🛠️        |
| prefer-to-have-length                | jest       |         |          | 🛠️        |
| prefer-todo                          | jest       |         |          | 🛠️        |
| require-hook                         | jest       |         |          |          |
| require-top-level-describe           | jest       |         |          |          |
| global-require                       | node       |         | ✅       |          |
| no-exports-assign                    | node       |         | ✅       | 🛠️        |
| avoid-new                            | promise    |         | ✅       |          |
| no-nesting                           | promise    |         | ✅       | 🚧       |
| no-return-wrap                       | promise    |         | ✅       | 🚧       |
| param-names                          | promise    |         | ✅       |          |
| prefer-await-to-callbacks            | promise    |         | ✅       |          |
| prefer-await-to-then                 | promise    |         | ✅       |          |
| prefer-catch                         | promise    |         | ✅       | 🚧       |
| hook-use-state                       | react      |         |          | 🚧       |
| jsx-boolean-value                    | react      |         |          | 🛠️        |
| jsx-curly-brace-presence             | react      |         |          | 🛠️        |
| jsx-fragments                        | react      |         |          | 🛠️        |
| jsx-handler-names                    | react      |         |          |          |
| jsx-max-depth                        | react      |         |          |          |
| jsx-pascal-case                      | react      |         |          |          |
| jsx-props-no-spreading               | react      |         |          |          |
| no-redundant-should-component-update | react      |         |          |          |
| no-set-state                         | react      |         |          |          |
| prefer-es6-class                     | react      |         |          |          |
| self-closing-comp                    | react      |         |          | 🛠️        |
| state-in-constructor                 | react      |         |          |          |
| adjacent-overload-signatures         | typescript |         | ✅       |          |
| array-type                           | typescript |         | ✅       | 🛠️        |
| ban-tslint-comment                   | typescript |         | ✅       | 🛠️        |
| class-literal-property-style         | typescript |         | ✅       | 🚧       |
| consistent-generic-constructors      | typescript |         | ✅       | 🛠️        |
| consistent-indexed-object-style      | typescript |         | ✅       | 🛠️        |
| consistent-type-assertions           | typescript |         | ✅       | 🛠️💡      |
| consistent-type-definitions          | typescript |         | ✅       | ⚠️🛠️️       |
| consistent-type-exports              | typescript |         | ✅       |          |
| consistent-type-imports              | typescript |         | ✅       | 🛠️        |
| dot-notation                         | typescript |         | ✅       |          |
| no-empty-interface                   | typescript |         | ✅       | 🚧       |
| no-inferrable-types                  | typescript |         | ✅       | 💡       |
| no-unnecessary-qualifier             | typescript |         | ✅       |          |
| parameter-properties                 | typescript |         | ✅       |          |
| prefer-find                          | typescript |         | ✅       |          |
| prefer-for-of                        | typescript |         | ✅       | 🚧       |
| prefer-function-type                 | typescript |         | ✅       | 🛠️        |
| prefer-readonly                      | typescript |         | ✅       |          |
| prefer-reduce-type-parameter         | typescript |         | ✅       | 🛠️        |
| prefer-regexp-exec                   | typescript |         | ✅       |          |
| prefer-return-this-type              | typescript |         | ✅       | 🛠️        |
| prefer-string-starts-ends-with       | typescript | ✅      | ✅       |          |
| unified-signatures                   | typescript |         | ✅       |          |
| catch-error-name                     | unicorn    |         | ✅       | 🛠️        |
| consistent-date-clone                | unicorn    |         | ✅       | 🛠️        |
| consistent-existence-index-check     | unicorn    |         | ✅       | 🛠️        |
| consistent-template-literal-escape   | unicorn    |         | ✅       | 🛠️        |
| custom-error-definition              | unicorn    |         | ✅       | 🚧       |
| empty-brace-spaces                   | unicorn    |         | ✅       | 🛠️        |
| error-message                        | unicorn    |         | ✅       |          |
| filename-case                        | unicorn    |         | ✅       |          |
| no-array-method-this-argument        | unicorn    |         | ✅       | 🚧       |
| no-await-expression-member           | unicorn    |         | ✅       | ⚠️🛠️️       |
| no-console-spaces                    | unicorn    |         | ✅       | 🛠️        |
| no-nested-ternary                    | unicorn    |         | ✅       | 🛠️        |
| no-null                              | unicorn    |         | ✅       | ⚠️🛠️️       |
| no-unreadable-array-destructuring    | unicorn    |         | ✅       | 🚧       |
| no-useless-collection-argument       | unicorn    |         | ✅       | 💡       |
| no-zero-fractions                    | unicorn    |         | ✅       | 🛠️        |
| number-literal-case                  | unicorn    |         | ✅       | 🛠️        |
| numeric-separators-style             | unicorn    |         | ✅       | 🛠️        |
| prefer-array-index-of                | unicorn    |         | ✅       | 🚧       |
| prefer-bigint-literals               | unicorn    |         | ✅       | 🛠️        |
| prefer-class-fields                  | unicorn    |         | ✅       | 🛠️💡      |
| prefer-classlist-toggle              | unicorn    |         | ✅       | 🛠️        |
| prefer-default-parameters            | unicorn    |         | ✅       | 🛠️        |
| prefer-dom-node-text-content         | unicorn    |         | ✅       | 🛠️        |
| prefer-global-this                   | unicorn    |         | ✅       | 💡       |
| prefer-includes                      | unicorn    |         | ✅       | 💡       |
| prefer-keyboard-event-key            | unicorn    |         | ✅       | 🛠️        |
| prefer-logical-operator-over-ternary | unicorn    |         | ✅       | 💡       |
| prefer-modern-dom-apis               | unicorn    |         | ✅       | 💡       |
| prefer-negative-index                | unicorn    |         | ✅       | 🛠️        |
| prefer-object-from-entries           | unicorn    |         | ✅       | 🚧       |
| prefer-optional-catch-binding        | unicorn    |         | ✅       | 🛠️        |
| prefer-reflect-apply                 | unicorn    |         | ✅       | 💡       |
| prefer-response-static-json          | unicorn    |         | ✅       | 💡       |
| prefer-spread                        | unicorn    |         | ✅       | 🛠️        |
| prefer-string-raw                    | unicorn    |         | ✅       | 🛠️        |
| prefer-string-trim-start-end         | unicorn    |         | ✅       | 🛠️        |
| prefer-structured-clone              | unicorn    |         | ✅       | 💡       |
| prefer-ternary                       | unicorn    |         | ✅       | 🚧       |
| relative-url-style                   | unicorn    |         | ✅       | 🛠️💡      |
| require-array-join-separator         | unicorn    |         | ✅       | 🛠️        |
| require-module-attributes            | unicorn    |         | ✅       | 💡       |
| switch-case-braces                   | unicorn    |         | ✅       | 🛠️        |
| switch-case-break-position           | unicorn    |         | ✅       | 🚧       |
| text-encoding-identifier-case        | unicorn    |         | ✅       | 🛠️        |
| throw-new-error                      | unicorn    |         | ✅       | 🛠️        |
| consistent-test-filename             | vitest     |         |          |          |
| consistent-test-it                   | vitest     |         |          | 🛠️        |
| consistent-vitest-vi                 | vitest     |         |          | 🛠️        |
| max-expects                          | vitest     |         |          |          |
| max-nested-describe                  | vitest     |         |          |          |
| no-alias-methods                     | vitest     |         |          | 🛠️        |
| no-duplicate-hooks                   | vitest     |         |          |          |
| no-hooks                             | vitest     |         |          |          |
| no-identical-title                   | vitest     |         |          |          |
| no-import-node-test                  | vitest     |         |          | 💡       |
| no-importing-vitest-globals          | vitest     |         |          | 🛠️        |
| no-interpolation-in-snapshots        | vitest     |         |          |          |
| no-large-snapshots                   | vitest     |         |          |          |
| no-mocks-import                      | vitest     |         |          |          |
| no-restricted-matchers               | vitest     |         |          |          |
| no-restricted-vi-methods             | vitest     |         |          |          |
| no-test-prefixes                     | vitest     |         |          | 🛠️        |
| no-test-return-statement             | vitest     |         |          |          |
| no-unneeded-async-expect-function    | vitest     |         |          | 🛠️        |
| prefer-called-exactly-once-with      | vitest     |         |          | ⚠️🛠️️       |
| prefer-called-once                   | vitest     |         |          | 🛠️        |
| prefer-called-times                  | vitest     |         |          | 🛠️        |
| prefer-called-with                   | vitest     |         |          | 🛠️        |
| prefer-comparison-matcher            | vitest     |         |          | 🛠️        |
| prefer-describe-function-title       | vitest     |         |          | 🛠️        |
| prefer-each                          | vitest     |         |          |          |
| prefer-equality-matcher              | vitest     |         |          | 💡       |
| prefer-expect-assertions             | vitest     |         |          | 💡       |
| prefer-expect-resolves               | vitest     |         |          | 🛠️        |
| prefer-expect-type-of                | vitest     |         |          | 🛠️        |
| prefer-hooks-in-order                | vitest     |         |          |          |
| prefer-hooks-on-top                  | vitest     |         |          |          |
| prefer-import-in-mock                | vitest     |         |          | 🛠️        |
| prefer-importing-vitest-globals      | vitest     |         |          | 🛠️        |
| prefer-lowercase-title               | vitest     |         |          | 🛠️        |
| prefer-mock-promise-shorthand        | vitest     |         |          | 🛠️        |
| prefer-mock-return-shorthand         | vitest     |         |          | 🛠️        |
| prefer-spy-on                        | vitest     |         |          | 💡       |
| prefer-strict-boolean-matchers       | vitest     |         |          | 🛠️        |
| prefer-strict-equal                  | vitest     |         |          | 🛠️        |
| prefer-to-be                         | vitest     |         |          | 🛠️        |
| prefer-to-be-falsy                   | vitest     |         |          | 🛠️        |
| prefer-to-be-object                  | vitest     |         |          | 🛠️        |
| prefer-to-be-truthy                  | vitest     |         |          | 🛠️        |
| prefer-to-contain                    | vitest     |         |          | 🛠️        |
| prefer-to-have-been-called-times     | vitest     |         |          | 🛠️        |
| prefer-to-have-length                | vitest     |         |          | 🛠️        |
| prefer-todo                          | vitest     |         |          | 🛠️        |
| require-hook                         | vitest     |         |          |          |
| require-top-level-describe           | vitest     |         |          |          |
| define-emits-declaration             | vue        |         |          | 🚧       |
| define-props-declaration             | vue        |         |          |          |
| define-props-destructuring           | vue        |         |          |          |
| require-typed-ref                    | vue        |         |          |          |

## Nursery (10)

New lints that are still under development.

| Rule name                    | Source     | Default | Enabled? | Fixable? |
| ---------------------------- | ---------- | ------- | -------- | -------- |
| no-restricted-exports        | eslint     |         | ✅       |          |
| no-undef                     | eslint     |         | ✅       |          |
| no-useless-assignment        | eslint     |         | ✅       |          |
| export                       | import     |         | ✅       |          |
| named                        | import     |         | ✅       |          |
| no-return-in-finally         | promise    |         | ✅       |          |
| require-render-return        | react      |         |          |          |
| no-unnecessary-condition     | typescript |         | ✅       |          |
| prefer-optional-chain        | typescript |         | ✅       | ⚠️🛠️️💡     |
| no-useless-iterator-to-array | unicorn    |         | ✅       | 🛠️💡      |

Default: 110
Total: 785
