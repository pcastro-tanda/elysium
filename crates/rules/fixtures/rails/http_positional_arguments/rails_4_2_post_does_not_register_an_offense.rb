post :create,
     id: @user.id,
     ac: {
       article_id: @article1.id,
       profile_id: @profile1.id,
       content: 'Some Text'
     }
