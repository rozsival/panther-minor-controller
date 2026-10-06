export default {
  overrides: [
    {
      // Shell sources only, so the plugin's Dockerfile support and the ```bash usage blocks in Markdown stay
      // untouched. The options give the output of `shfmt -i 2`.
      files: ['*.sh'],
      options: {
        binaryNextLine: false,
        plugins: ['prettier-plugin-sh'],
        spaceRedirects: false,
        switchCaseIndent: false
      }
    }
  ],
  plugins: ['prettier-plugin-packagejson', 'prettier-plugin-toml'],
  printWidth: 120,
  singleQuote: true,
  trailingComma: 'none'
};
