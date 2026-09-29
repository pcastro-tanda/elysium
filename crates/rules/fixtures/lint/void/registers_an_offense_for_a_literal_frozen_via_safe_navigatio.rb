def something
  'foo'&.freeze
  ^^^^^^^^^^^^^ Literal `'foo'&.freeze` used in void context.
  baz
end
