class Article
  belongs_to :user
  validates :user, uniqueness: true
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
