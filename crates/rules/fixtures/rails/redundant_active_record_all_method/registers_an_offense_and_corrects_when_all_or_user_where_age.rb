User.all.or(User.where(age: 30))
     ^^^ Redundant `all` detected.
