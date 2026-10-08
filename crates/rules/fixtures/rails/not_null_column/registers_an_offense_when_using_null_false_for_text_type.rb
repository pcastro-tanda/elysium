def change
  add_column :articles, :content, :text, null: false
                                         ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
end
