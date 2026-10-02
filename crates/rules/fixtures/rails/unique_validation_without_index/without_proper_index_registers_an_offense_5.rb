class Article
  belongs_to :member, foreign_key: :user_id
  validates :member, uniqueness: true
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Uniqueness validation should have a unique index on the database column.
end
