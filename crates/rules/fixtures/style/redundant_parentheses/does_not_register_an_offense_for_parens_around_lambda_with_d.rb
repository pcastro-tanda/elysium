scope :my_scope, (lambda do
  where(column: :value)
end)
