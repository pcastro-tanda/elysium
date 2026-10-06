class Article
  belongs_to :user
  validates :user, uniqueness: true, if: -> { false }
end
