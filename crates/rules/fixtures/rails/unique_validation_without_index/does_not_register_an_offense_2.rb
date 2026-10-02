class WrittenArticles
  belongs_to :author
  validates :title, uniqueness: { scope: :author }
end
