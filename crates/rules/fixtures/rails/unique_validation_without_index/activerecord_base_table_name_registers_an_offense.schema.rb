ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "members", force: :cascade do |t|
    t.string "account", null: false
  end
end
