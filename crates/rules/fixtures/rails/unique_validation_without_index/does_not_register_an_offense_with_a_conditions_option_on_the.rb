class Article
  belongs_to :user
  enum :status, [:draft, :published]
  validates :user, uniqueness: { conditions: -> { published } }
end
