class Article
  belongs_to :user
  validates :user, uniqueness: { if: -> { false } }
end
