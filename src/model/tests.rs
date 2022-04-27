use super::{CommentJson, Comment, Comments, FileName, LineNumber, Url, User, CommentId, FileCommentsJson, LineCommentsJson};

#[test]
fn comment_json_grouped_by_line() {

  let comment1 =
    Comment {
      comment_id: CommentId::new(1000),
      diff_hunk: "diff hunk1".to_owned(),
      body: "body1".to_owned(),
      author: User::new("user1".to_owned(), Url::new("https://sample.data/user1".to_owned())),
      comment_url: Url::new("https://sample.data/comment1".to_owned()),
      line: Some(LineNumber::new(100)),
      file_name: FileName::new("filename1".to_owned()),
      in_reply_to_id: None
    };

  let comments =
    Comments {
      comments: vec![comment1]
    };

  let actual_result = CommentJson::grouped_by_line(comments);

  let expected_comment_json =
    CommentJson {
      user_name: "user1".to_owned(),
      user_icon: "https://sample.data/user1".to_owned(),
      link: "https://sample.data/comment1".to_owned(),
      line: 100,
      body: "body1".to_owned(),
      file_name: "filename1".to_owned(),
    };

  let expected_line_comments =
    LineCommentsJson {
      line: 100,
      file_name: "filename1".to_owned(),
      comments: vec![expected_comment_json]
    };


  let expected_results =
    vec![
      FileCommentsJson {
        file_name: "filename1".to_owned(),
        comments: vec![expected_line_comments]
      }
    ];

  assert_eq!(expected_results, actual_result);
}


