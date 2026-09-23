scope :my_scope, (proc do
  where(column: :value)
end)
