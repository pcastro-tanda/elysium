A = Regexp.union(
  /[A-Za-z_][A-Za-z\d_]*[!?=]?/,
  *AST::Types::OPERATOR_METHODS.map(&:to_s)
).freeze
