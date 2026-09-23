scope :my_scope, (lambda {
                 ^^^^^^^^^ Don't use parentheses around an expression.
  where(column: :value)
})
