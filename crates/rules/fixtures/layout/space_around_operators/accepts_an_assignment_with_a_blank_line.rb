expected = posts(:welcome)

tagging  = Tagging.all.merge!(includes: :taggable).find(taggings(:welcome_general).id)
assert_no_queries { assert_equal expected, tagging.taggable }
