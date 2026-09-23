scope :my_scope, (-> do
                 ^^^^^^ Don't use parentheses around an expression.
  where(column: :value)
end)
