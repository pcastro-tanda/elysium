class WrittenArticles
  belongs_to :author, polymorphic: false
  validates :title, uniqueness: { scope: :author }
end
