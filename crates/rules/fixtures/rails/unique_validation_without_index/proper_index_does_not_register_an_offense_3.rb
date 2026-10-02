class WrittenArticles
  validates :a_id, uniqueness: { scope: [:b_id, :c_id] }
end
