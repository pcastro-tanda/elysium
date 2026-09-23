scope :my_scope, (proc {
                 ^^^^^^^ Don't use parentheses around an expression.
  where(column: :value)
})
