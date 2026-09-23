scope :my_scope, -> do
  where(column: :value)
end
