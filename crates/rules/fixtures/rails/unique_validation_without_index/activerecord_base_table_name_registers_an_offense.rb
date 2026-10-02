class User
  self.table_name = 'members'
  validates :account, uniqueness: true
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
