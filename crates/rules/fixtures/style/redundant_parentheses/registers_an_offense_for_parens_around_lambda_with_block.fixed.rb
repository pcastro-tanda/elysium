scope :my_scope, lambda {
  where(column: :value)
}
