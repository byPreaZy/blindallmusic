import defaultTheme from 'tailwindcss/defaultTheme';

const withAlpha = (variable) => `rgb(var(${variable}) / <alpha-value>)`;

/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.rs',
    './css/**/*.css',
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Space Grotesk', ...defaultTheme.fontFamily.sans],
      },
      borderRadius: {
        squircle: '1.5rem',
        'squircle-sm': '1.25rem',
        'squircle-xs': '1rem',
      },
      colors: {
        brand: {
          midnight: withAlpha('--color-midnight'),
          deep: withAlpha('--color-deep'),
          'deep-hover': withAlpha('--color-deep-hover'),
          'deep-card': withAlpha('--color-deep-card'),
          primary: withAlpha('--color-primary'),
          'primary-hover': withAlpha('--color-primary-hover'),
          'primary-dark': withAlpha('--color-primary-dark'),
          'primary-light': withAlpha('--color-primary-light'),
          secondary: withAlpha('--color-secondary'),
          'secondary-dark': withAlpha('--color-secondary-dark'),
          'secondary-strong': withAlpha('--color-secondary-strong'),
          accent: withAlpha('--color-accent'),
          'accent-hover': withAlpha('--color-accent-hover'),
          'accent-dark': withAlpha('--color-accent-dark'),
          'accent-light': withAlpha('--color-accent-light'),
          danger: withAlpha('--color-danger'),
          'danger-dark': withAlpha('--color-danger-dark'),
        },
        red: {
          DEFAULT: '#F24B43',
          50: '#fef0ef',
          100: '#fbdedc',
          200: '#f7bdba',
          300: '#f2918d',
          400: withAlpha('--color-primary-light'),
          500: withAlpha('--color-primary'),
          600: withAlpha('--color-primary-dark'),
          700: '#8f2721',
          800: '#6e1e1a',
          900: '#4c1512',
          950: '#330d0b',
        },
        teal: {
          DEFAULT: '#F66059',
          400: withAlpha('--color-accent-hover'),
          500: withAlpha('--color-accent'),
          600: withAlpha('--color-accent-dark'),
          700: '#8f2721',
          800: '#611b16',
          900: '#3f1210',
          950: '#290b0a',
        },
        yellow: {
          DEFAULT: '#F2EEE6',
          400: withAlpha('--color-secondary'),
          500: withAlpha('--color-secondary-dark'),
          600: '#bab2a4',
        },
        orange: {
          400: withAlpha('--color-primary-light'),
          500: withAlpha('--color-primary'),
          600: withAlpha('--color-primary-dark'),
          700: '#8f2721',
          950: '#330d0b',
        },
        emerald: {
          300: withAlpha('--color-accent-hover'),
          400: withAlpha('--color-accent-hover'),
          500: withAlpha('--color-accent'),
          600: withAlpha('--color-accent-dark'),
        },
        indigo: {
          400: withAlpha('--color-accent-hover'),
          500: withAlpha('--color-accent'),
          600: withAlpha('--color-accent-dark'),
          700: '#8f2721',
          800: '#611b16',
          900: withAlpha('--color-deep'),
        },
        surface: {
          DEFAULT: withAlpha('--color-deep-card'),
          base: withAlpha('--color-midnight'),
          raised: withAlpha('--color-deep'),
          overlay: withAlpha('--color-deep-card'),
          hover: withAlpha('--color-deep-hover'),
        },
        arena: {
          base: withAlpha('--color-midnight'),
          panel: withAlpha('--color-deep'),
          card: withAlpha('--color-deep-card'),
          hover: withAlpha('--color-deep-hover'),
        },
      },
      keyframes: {
        shaking: {
          '0%': { transform: 'translate(0, 0) rotate(0deg)' },
          '25%': { transform: 'translate(2px, 2px) rotate(0.1deg)' },
          '50%': { transform: 'translate(0, 0) rotate(0eg)' },
          '75%': { transform: 'translate(-2px, 2px) rotate(-0.1deg)' },
          '100%': { transform: 'translate(0, 0) rotate(0deg)' },
        },
      },
      animation: {
        shake: 'shaking .2s cubic-bezier(.36,.07,.19,.97) infinite',
      },
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
};
