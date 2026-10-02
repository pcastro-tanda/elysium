ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "written_articles", force: :cascade do |t|
    t.bigint "user_id", null: false
    t.bigint "article_id", null: false
    t.index ["user_id"], name: "idx_uid", unique: true
    t.index ["article_id"], name: "idx_aid", unique: true
  end
end
