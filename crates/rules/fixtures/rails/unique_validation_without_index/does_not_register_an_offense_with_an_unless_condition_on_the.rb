class Article
  belongs_to :user
  validates :user, uniqueness: { unless: -> { false } }
end
