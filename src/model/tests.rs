use std::collections::HashMap;

use crate::model::{Base64Encoded, Markdown};

use super::{CommentJson, Comment, Comments, FileName, LineNumber, Url, User, UserId, CommentId, FileCommentsJson, LineCommentsJson};

#[test]
fn comment_json_grouped_by_line() {

  let comment1 =
    Comment {
      comment_id: CommentId::new(1000),
      diff_hunk: "diff hunk1".to_owned(),
      body: "body1".to_owned(),
      markdown_body: None,
      author: User::new("user1".to_owned(), Url::new("https://sample.data/user1".to_owned()), UserId::new(1)),
      comment_url: Url::new("https://sample.data/comment1".to_owned()),
      line: Some(LineNumber::new(100)),
      file_name: FileName::new("filename1".to_owned()),
      in_reply_to_id: None
    };

  let comment2 =
    Comment {
      comment_id: CommentId::new(1002),
      diff_hunk: "diff hunk2".to_owned(),
      body: "body2".to_owned(),
      markdown_body: None,
      author: User::new("user2".to_owned(), Url::new("https://sample.data/user2".to_owned()), UserId::new(2)),
      comment_url: Url::new("https://sample.data/comment2".to_owned()),
      line: Some(LineNumber::new(150)),
      file_name: FileName::new("filename2".to_owned()),
      in_reply_to_id: None
    };

  let comment3 =
    Comment {
      comment_id: CommentId::new(1003),
      diff_hunk: "diff hunk3".to_owned(),
      body: "body3".to_owned(),
      markdown_body: None,
      author: User::new("user3".to_owned(), Url::new("https://sample.data/user3".to_owned()), UserId::new(3)),
      comment_url: Url::new("https://sample.data/comment3".to_owned()),
      line: Some(LineNumber::new(100)),
      file_name: FileName::new("filename1".to_owned()),
      in_reply_to_id: Some(CommentId::new(1000))
    };

  let comment4 =
    Comment {
      comment_id: CommentId::new(1004),
      diff_hunk: "diff hunk4".to_owned(),
      body: "body4".to_owned(),
      markdown_body: None,
      author: User::new("user4".to_owned(), Url::new("https://sample.data/user4".to_owned()), UserId::new(4)),
      comment_url: Url::new("https://sample.data/comment4".to_owned()),
      line: None,
      file_name: FileName::new("filename1".to_owned()),
      in_reply_to_id: Some(CommentId::new(1000))
    };

  let comment5 =
    Comment {
      comment_id: CommentId::new(1005),
      diff_hunk: "diff hunk5".to_owned(),
      body: "body5".to_owned(),
      markdown_body: Some(Markdown::new("**markdown**".to_owned())),
      author: User::new("user5".to_owned(), Url::new("https://sample.data/user5".to_owned()), UserId::new(5)),
      comment_url: Url::new("https://sample.data/comment5".to_owned()),
      line: Some(LineNumber::new(30)),
      file_name: FileName::new("filename3".to_owned()),
      in_reply_to_id: None
    };

  let comments =
    Comments {
      comments:
        vec![
        comment1,
        comment2,
        comment3,
        comment4,
        comment5,
        ]
    };

  let avatar_hash =
    HashMap::from([
      (Url::new("https://sample.data/user1".to_owned()), Base64Encoded::new("https://sample.data/user1".to_owned())),
      (Url::new("https://sample.data/user2".to_owned()), Base64Encoded::new("https://sample.data/user2".to_owned())),
      (Url::new("https://sample.data/user5".to_owned()), Base64Encoded::new("https://sample.data/user5".to_owned())),
    ]);

  let mut actual_result = CommentJson::grouped_by_line_2(comments, avatar_hash);

  let expected_comment_json1 =
    CommentJson {
      user_name: "user1".to_owned(),
      user_icon: "https://sample.data/user1".to_owned(),
      link: "https://sample.data/comment1".to_owned(),
      line: 100,
      body: "body1".to_owned(),
      body_md: None,
      file_name: "filename1".to_owned(),
    };

  let expected_comment_json2 =
    CommentJson {
      user_name: "user2".to_owned(),
      user_icon: "https://sample.data/user2".to_owned(),
      link: "https://sample.data/comment2".to_owned(),
      line: 150,
      body: "body2".to_owned(),
      body_md: None,
      file_name: "filename2".to_owned(),
    };

  let expected_comment_json3 =
    CommentJson {
      user_name: "user3".to_owned(),
      user_icon: "-".to_owned(), // since this is not in the cache we get the default
      link: "https://sample.data/comment3".to_owned(),
      line: 100,
      body: "body3".to_owned(),
      body_md: None,
      file_name: "filename1".to_owned(),
    };

  //comment4 should be filtered out

  let expected_comment_json5 =
    CommentJson {
      user_name: "user5".to_owned(),
      user_icon: "https://sample.data/user5".to_owned(),
      link: "https://sample.data/comment5".to_owned(),
      line: 30,
      body: "body5".to_owned(),
      body_md: Some("**markdown**".to_owned()), //since comment5 has markdown, we expect it to be output
      file_name: "filename3".to_owned(),
    };



  let expected_line_comments_file1 =
    LineCommentsJson {
      line: 100,
      file_name: "filename1".to_owned(),
      file_line_comments:
        vec![
          expected_comment_json1,
          expected_comment_json3,
        ]
    };

  let expected_line_comments_file2 =
    LineCommentsJson {
      line: 150,
      file_name: "filename2".to_owned(),
      file_line_comments:
        vec![
          expected_comment_json2,
        ]
    };

  let expected_line_comments_file3 =
    LineCommentsJson {
      line: 30,
      file_name: "filename3".to_owned(),
      file_line_comments:
        vec![
          expected_comment_json5,
        ]
    };


  let expected_results =
    vec![
      FileCommentsJson {
        file_name: "filename1".to_owned(),
        file_comments: vec![expected_line_comments_file1]
      },
      FileCommentsJson {
        file_name: "filename2".to_owned(),
        file_comments: vec![expected_line_comments_file2]
      },
      FileCommentsJson {
        file_name: "filename3".to_owned(),
        file_comments: vec![expected_line_comments_file3]
      },
    ];

  //Sort so we have a predictable ordering
  actual_result.sort_by(|a, b| a.file_name.partial_cmp(&b.file_name).unwrap());
  assert_eq!(expected_results, actual_result);
}


