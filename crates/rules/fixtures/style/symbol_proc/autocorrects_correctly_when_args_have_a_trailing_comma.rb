mail(
  to: 'foo',
  subject: 'bar',
) { |format| format.text }
  ^^^^^^^^^^^^^^^^^^^^^^^^ Pass `&:text` as an argument to `mail` instead of a block.
