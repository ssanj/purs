use super::{CommentJson, Comment, Comments, FileName, LineNumber, Url, User, CommentId};

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

  let result = CommentJson::grouped_by_line(comments);

  assert_eq!(result.len(), 1, "length for result: {:?}", result);
  let item = result.get(0).unwrap();

  assert_eq!(item.file_name, "filename1".to_owned());
  let line_comments = &item.comments;
  assert_eq!(line_comments.len(), 1, "length for line_comments: {:?}", line_comments);

  let line_comment = line_comments.get(0).unwrap();

  assert_eq!(line_comment.line, 100);
  assert_eq!(line_comment.file_name, "filename1".to_owned());

  let comments = &line_comment.comments;
  assert_eq!(comments.len(), 1, "length for comments: {:?}", line_comments);

  let comment = comments.get(0).unwrap();
  assert_eq!(comment.line, 100);
  assert_eq!(comment.file_name, "filename1".to_owned());
  assert_eq!(comment.user_name , "user1".to_owned());
  assert_eq!(comment.user_icon , "https://sample.data/user1".to_owned());
  assert_eq!(comment.body , "body1".to_owned());
  assert_eq!(comment.link , "https://sample.data/comment1".to_owned());
}


