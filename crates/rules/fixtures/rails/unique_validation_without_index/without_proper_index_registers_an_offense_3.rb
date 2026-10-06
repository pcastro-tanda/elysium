class WrittenArticles
  validates :a_id, uniqueness: { scope: [:b_id, :c_id] }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
