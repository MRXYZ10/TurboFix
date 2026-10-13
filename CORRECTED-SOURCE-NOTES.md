# TurboFix corrected source notes

This package contains the complete TurboFix source project, including the updated speed-test component, guarded history refresh in `src/App.tsx`, and the app tests in `src/__tests__/app.test.tsx`.

## Fresh setup on Windows

Open PowerShell in this `TurboFix` folder and run:

```powershell
npm ci
npm test
npm run build
```

`npm ci` requires a working connection to the npm registry. Rust/MSVC build tools are required separately for building the native Windows/Tauri app.

The source has not been executed through Vitest in this packaging environment because npm registry requests failed. Run the commands above locally and use the output to confirm the test suite result.
