class WrittenArticles
  belongs_to :author, polymorphic: true
  validates :title, uniqueness: { scope: :author }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
