def change
  %w[owners members].each do |table|
    add_column table, :name, :string, null: false
  end
end
