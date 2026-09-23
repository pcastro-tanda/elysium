scope :my_scope, proc {
  where(column: :value)
}
