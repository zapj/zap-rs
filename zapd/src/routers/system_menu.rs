use axum::Json;
use serde_json::json;

use crate::zap::ZapJsonResult;



pub async fn get_menus_tree() -> ZapJsonResult {

    Ok(Json(json!({
      "code":0,
      "message":"ok",
      "data":[
            {
              "id": 1,
              "name": "dashboard",
              "path": "/dashboard",
              "component": "Layout",
              "redirect": "/dashboard/analysis",
              "type": "menu",
              "meta": {
                "title": "仪表盘",
                "icon": "ep:menu",
                "roles": ["admin", "editor", "user"], // 所有角色可见
                "affix": true,
              },
              "order": 1,
              "status": 1, // 不隐藏
            },
            {
                "id": 2,
                "name": "system",
                "path": "/system",
                "component": "Layout",
                "redirect": "/system/user",
                "type": "dir",
                "meta": {
                    "title": "系统管理",
                    "icon": "ep:setting",
                    "roles": ["admin"], // 仅管理员可见
                    "affix": true,
                },
                "order": 2,
                "status": 1, // 不隐藏
                "children": [
                    {
                        "id": 21,
                        "name": "user",
                        "path": "user",
                        "component": "system/users/index",
                        "type": "menu",
                        "meta": {
                            "title": "用户管理",
                            "icon": "ep:user",
                            "roles": ["admin"], // 仅管理员可见
                            "affix":true,
                    },
                    "order": 1,
                    "status": 1, // 不隐藏
                },
                {
                    "id": 22,
                    "name": "roles",
                    "path": "roles",
                    "component": "system/roles/index",
                    "type": "menu",
                    "meta": {
                        "title": "角色管理",
                        "icon": "ep:view",
                        "roles": ["admin"], // 仅管理员可见
                        "affix":true,
                  },
                  "order": 2,
                  "status": 1, // 不隐藏
                },
                {
                    "id": 23,
                    "name": "menus",
                    "path": "menus",
                    "component": "system/menus/index",
                    "type": "menu",
                    "meta": {
                        "title": "菜单管理",
                        "icon": "ep:menu",
                        "roles": ["admin"], // 仅管理员可见
                        "affix": true,
                  },
                  "order": 3,
                  "status": 1, // 不隐藏
          
                },
              ],
            },
            {
                "id": 3,
                "name": "content",
                "path": "/content",
                "component": "Layout",
                "redirect": "/content/articles",
                "type": "dir",
                "meta": {
                    "title": "内容管理",
                    "icon": "ep:document",
                    "roles": ["admin", "editor"], // 管理员和编辑可见
                    "affix": true,
                },
                "order": 3,
                "status": 1, // 不隐藏
                "children": [
                {
                    "id": 31,
                    "name": "articles",
                    "path": "articles",
                    "component": "content/articles/index",
                    "type": "menu",
                    "meta": {
                        "title": "文章管理",
                        "icon": "ep:document",
                        "roles": ["admin", "editor"], // 管理员和编辑可见
                        "affix": true,
                  },
                  "order": 1,
                  "status": 1, // 不隐藏
                },
                {
                    "id": 32,
                    "name": "categories",
                    "path": "categories",
                    "component": "content/categories/index",
                    "type": "menu",
                    "meta": {
                        "title": "分类管理",
                        "icon": "ep:folder",
                        "roles": ["admin", "editor"], // 管理员和编辑可见
                        "affix": true,
                  },
                  "order": 2,
                  "status": 1, // 不隐藏
                },
                {
                    "id": 33,
                    "name": "tags",
                    "path": "tags",
                    "component": "content/tags/index",
                    "type": "menu",
                    "meta": {
                        "title": "标签管理",
                        "icon": "ep:star",
                        "roles": ["admin", "editor"], // 管理员和编辑可见
                        "affix": true,
                    },
                    "order": 3,
                    "status": 1, // 不隐藏
                },
              ],
            },
          ]
    })))
}