class Article
  belongs_to :user
  validates :user, uniqueness: true, unless: -> { true }
end
