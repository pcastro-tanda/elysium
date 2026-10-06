ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "written_articles", force: :cascade do |t|
    t.string "title", null: false
    t.string "author_type", null: false
    t.bigint "author_id", null: false
    t.index ["title", "author_id", "author_type"], name: "idx_aid", unique: true
  end
end
