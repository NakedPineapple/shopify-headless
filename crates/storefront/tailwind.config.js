/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.html",
    "./src/**/*.rs",
  ],
  theme: {
    extend: {
      // TODO: Add brand colors, fonts, and other customizations
      // colors: {
      //   brand: {
      //     50: '#...',
      //     ...
      //   },
      // },
      // fontFamily: {
      //   sans: ['Inter', 'sans-serif'],
      //   display: ['Playfair Display', 'serif'],
      // },
    },
  },
  plugins: [
    // TODO: Add plugins as needed
    // require('@tailwindcss/forms'),
    // require('@tailwindcss/typography'),
    // require('@tailwindcss/aspect-ratio'),
  ],
}
