User.find_by_sql(["select * from users where name = ?", name])
