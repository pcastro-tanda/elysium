user.posts.exists?(published: true)
           ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `where(published: true).exists?` over `exists?(published: true)`.
