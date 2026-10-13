# Test selector fixes (0.3.1 source patch)

The app navigation labels use sentence case (for example, `Storage cleanup`, `Smart optimizer`, `Reports & history`, `Speed test`, and `Wi-Fi analysis`). Several UI tests were still querying older title-case or placeholder labels. Updated those tests to use case-insensitive accessible-name matchers for the current visible navigation labels.

These changes update test selectors only; they do not weaken assertions or change application behavior. Run `npm ci` followed by `npm test` and `npm run build` on a development machine to verify the full suite.
