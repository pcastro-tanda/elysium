ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "articles", force: :cascade do |t|
    t.bigint "foo_id", null: false
    t.index ["user_id"], name: "idx_user_id", unique: true
  end
end
