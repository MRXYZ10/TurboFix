/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { 950: "#0b0e14", 900: "#10141c", 800: "#171c27", 700: "#222a38", 600: "#2f394b" },
        accent: { DEFAULT: "#22d3ee", soft: "#0e7490", blue: "#3b82f6" },
      },
      fontFamily: { sans: ["Segoe UI Variable", "Segoe UI", "system-ui", "sans-serif"] },
    },
  },
  plugins: [],
};
