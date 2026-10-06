user.posts.where(published: true).exists?
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `exists?(published: true)` over `where(published: true).exists?`.
