class WrittenArticles
  validates :user_id, uniqueness: { scope: :article_id }
end
