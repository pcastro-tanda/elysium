create_table :users, :articles, id: false do |t|
  t.integer :user_id
  t.integer :article_id
end
