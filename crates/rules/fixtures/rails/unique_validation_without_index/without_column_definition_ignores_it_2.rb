class Article
  belongs_to :member, foreign_key: :user_id
  validates :member, uniqueness: true
end
