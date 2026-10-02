User.all.eager_load(:articles)
     ^^^ Redundant `all` detected.
