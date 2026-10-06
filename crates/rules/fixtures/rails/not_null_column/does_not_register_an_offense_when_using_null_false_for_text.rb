def change
  add_column :articles, :content, :text, null: false
end
