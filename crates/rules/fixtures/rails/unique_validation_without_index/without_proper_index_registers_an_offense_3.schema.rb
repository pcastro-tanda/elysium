ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "written_articles", force: :cascade do |t|
    t.bigint "a_id", null: false
    t.bigint "b_id", null: false
    t.bigint "c_id", null: false
    t.index ["a_id"], name: "idx_aid", unique: true
    t.index ["b_id"], name: "idx_bid", unique: true
    t.index ["c_id"], name: "idx_cid", unique: true
  end
end
