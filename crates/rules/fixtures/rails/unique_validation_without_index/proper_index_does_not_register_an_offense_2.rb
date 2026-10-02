class WrittenArticles
  belongs_to :author, polymorphic: true
  validates :title, uniqueness: { scope: :author }
end
